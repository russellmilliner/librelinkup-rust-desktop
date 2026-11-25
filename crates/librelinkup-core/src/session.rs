//! Shared CGM session polling, automatic token re-authentication, alert evaluation,
//! snooze formatting, and demo telemetry generation used by both KDE Plasma and Windows.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use chrono::{Duration as ChronoDuration, Local, TimeZone};

use crate::api::{
    ApiError, HaPatientOption, authenticate_async, discover_ha_patients,
    fetch_cgm_snapshot_with_patient_hint_async, fetch_ha_cgm_snapshot_and_states_async,
    fetch_ha_sync_states_async, parse_ha_remote_snooze,
};
use crate::crypto::StoredCredentials;
use crate::event_log::{log_info, log_warn};
use crate::medications::{MedicationReminder, MedicationStore};
use crate::models::{
    AlertKind, AppSettings, DEFAULT_TARGET_HIGH_MG_DL, DEFAULT_TARGET_LOW_MG_DL, GlucoseSnapshot,
    GraphPoint, format_glucose_value, format_reading_age, reading_age_minutes, trend_arrow_symbol,
};

#[derive(Debug, Clone)]
pub struct RefreshOutcome {
    pub snapshot: GlucoseSnapshot,
    /// Present when credentials were updated (e.g. token refreshed after 401 or patient metadata populated).
    pub updated_credentials: Option<StoredCredentials>,
    /// Optional Home Assistant `/api/states` array fetched during this poll cycle for two-way
    /// medication & cross-device alert snooze synchronization.
    pub ha_states: Option<Vec<serde_json::Value>>,
}

// ---------------------------------------------------------------------------
// Re-authentication backoff
// ---------------------------------------------------------------------------

/// Consecutive failed re-authentication attempts and the earliest epoch at which another
/// attempt is allowed. Prevents an expired token plus a wrong stored password from
/// burning one extra login (plus two requests) on every 60-second poll.
#[derive(Debug, Default)]
struct ReauthBackoff {
    failures: u32,
    next_allowed_epoch: i64,
}

fn reauth_state() -> &'static Mutex<ReauthBackoff> {
    static STATE: OnceLock<Mutex<ReauthBackoff>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(ReauthBackoff::default()))
}

fn lock_reauth() -> std::sync::MutexGuard<'static, ReauthBackoff> {
    reauth_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Returns `true` when a re-authentication attempt is allowed at `now_epoch`.
pub fn reauth_allowed(now_epoch: i64) -> bool {
    now_epoch >= lock_reauth().next_allowed_epoch
}

/// Records a successful re-authentication, clearing the backoff.
pub fn record_reauth_success() {
    let mut state = lock_reauth();
    state.failures = 0;
    state.next_allowed_epoch = 0;
}

/// Records a failed re-authentication and returns the backoff applied, in seconds.
pub fn record_reauth_failure(now_epoch: i64) -> i64 {
    let mut state = lock_reauth();
    state.failures = state.failures.saturating_add(1);
    let delay_secs = (60i64 << state.failures.min(6)).min(3600);
    state.next_allowed_epoch = now_epoch + delay_secs;
    delay_secs
}

/// Clears the backoff state (used by tests and on explicit sign-out).
pub fn reset_reauth_backoff() {
    let mut state = lock_reauth();
    state.failures = 0;
    state.next_allowed_epoch = 0;
}

/// Fetches the latest CGM snapshot, automatically re-authenticating once if the bearer token
/// has expired (`401 Unauthorized`) and the user opted in to storing their account password.
pub async fn fetch_snapshot_with_reauth_async(
    creds: &StoredCredentials,
) -> Result<RefreshOutcome, ApiError> {
    let mut active_creds = creds.clone();
    let mut reauthenticated = false;

    let patient_hint =
        (!active_creds.patient_id.trim().is_empty()).then_some(active_creds.patient_id.as_str());
    let mut result = fetch_cgm_snapshot_with_patient_hint_async(
        &active_creds.country,
        &active_creds.token,
        &active_creds.account_id,
        patient_hint,
    )
    .await;

    if matches!(result, Err(ApiError::Unauthorized))
        && !active_creds.username.trim().is_empty()
        && !active_creds.password.is_empty()
    {
        let now = chrono::Utc::now().timestamp();
        if !reauth_allowed(now) {
            log_warn(
                "AUTH",
                "Bearer token expired but re-authentication is backing off after repeated failures.",
            );
        } else {
            log_info(
                "AUTH",
                "Bearer token expired (401); re-authenticating with stored credentials...",
            );
            match authenticate_async(
                &active_creds.country,
                &active_creds.username,
                &active_creds.password,
            )
            .await
            {
                Ok(mut reauth) => {
                    record_reauth_success();
                    active_creds.token = std::mem::take(&mut reauth.token);
                    active_creds.account_id = std::mem::take(&mut reauth.account_id);
                    active_creds.country = std::mem::take(&mut reauth.country);
                    reauthenticated = true;
                    let reauth_hint = (!active_creds.patient_id.trim().is_empty())
                        .then_some(active_creds.patient_id.as_str());
                    result = fetch_cgm_snapshot_with_patient_hint_async(
                        &active_creds.country,
                        &active_creds.token,
                        &active_creds.account_id,
                        reauth_hint,
                    )
                    .await;
                }
                Err(e) => {
                    let delay = record_reauth_failure(now);
                    log_warn(
                        "AUTH",
                        format!(
                            "Re-authentication failed ({e}); next attempt in {delay}s. Sign in again if this persists."
                        ),
                    );
                }
            }
        }
    }

    let snapshot = result?;
    let mut updated_credentials = None;
    if reauthenticated || active_creds.patient_id != snapshot.patient_id {
        active_creds.patient_id = snapshot.patient_id.clone();
        if !snapshot.first_name.is_empty() {
            active_creds.first_name = snapshot.first_name.clone();
        }
        if !snapshot.last_name.is_empty() {
            active_creds.last_name = snapshot.last_name.clone();
        }
        updated_credentials = Some(active_creds);
    }

    Ok(RefreshOutcome {
        snapshot,
        updated_credentials,
        ha_states: None,
    })
}

/// Fetches the latest CGM snapshot from either the Home Assistant LibreLink integration
/// (`data_source == "home_assistant"`) or the LibreLinkUp cloud API (`"librelinkup"`).
pub async fn fetch_snapshot_for_source_async(
    data_source: &str,
    ha_url: &str,
    ha_sensor_entity_id: &str,
    creds: &StoredCredentials,
) -> Result<RefreshOutcome, ApiError> {
    fetch_snapshot_for_source_with_sync_async(
        data_source,
        ha_url,
        ha_sensor_entity_id,
        creds,
        true,
        true,
    )
    .await
}

/// Fetches the latest CGM snapshot plus only the Home Assistant sync states enabled by
/// `sync_meds` / `sync_snooze` when running in LibreLinkUp mode.
pub async fn fetch_snapshot_for_source_with_sync_async(
    data_source: &str,
    ha_url: &str,
    ha_sensor_entity_id: &str,
    creds: &StoredCredentials,
    sync_meds: bool,
    sync_snooze: bool,
) -> Result<RefreshOutcome, ApiError> {
    if data_source.trim().eq_ignore_ascii_case("home_assistant") {
        let (snapshot, states) =
            fetch_ha_cgm_snapshot_and_states_async(ha_url, &creds.ha_token, ha_sensor_entity_id)
                .await?;
        let mut active_creds = creds.clone();
        let mut updated_credentials = None;
        if active_creds.patient_id != snapshot.patient_id
            || (!snapshot.first_name.is_empty() && active_creds.first_name != snapshot.first_name)
        {
            active_creds.patient_id = snapshot.patient_id.clone();
            if !snapshot.first_name.is_empty() {
                active_creds.first_name = snapshot.first_name.clone();
            }
            if !snapshot.last_name.is_empty() {
                active_creds.last_name = snapshot.last_name.clone();
            }
            updated_credentials = Some(active_creds);
        }
        Ok(RefreshOutcome {
            snapshot,
            updated_credentials,
            ha_states: Some(states),
        })
    } else {
        let mut outcome = fetch_snapshot_with_reauth_async(creds).await?;
        if (sync_meds || sync_snooze)
            && !ha_url.trim().is_empty()
            && !creds.ha_token.trim().is_empty()
        {
            outcome.ha_states =
                fetch_ha_sync_states_async(ha_url, &creds.ha_token, sync_meds, sync_snooze)
                    .await
                    .ok();
        }
        Ok(outcome)
    }
}

/// Effective age of the displayed reading in whole minutes.
///
/// A reading can be old in two ways: its own timestamp is old, or we have not managed to
/// fetch anything recently (no signal, server outage, suspended laptop). Taking the maximum
/// of the two makes staleness detection fail closed: a dead sensor on a fresh fetch keeps
/// its older reading age, and failed fetches age a once-fresh reading via the fetch clock.
/// If the reading itself lacks a parseable timestamp (`None`), returns `None` so callers
/// fail closed as stale even if a fetch just succeeded.
pub fn effective_reading_age_minutes(
    snapshot: &GlucoseSnapshot,
    now_epoch: i64,
    last_successful_fetch_epoch: Option<i64>,
) -> Option<i64> {
    let reading_age = reading_age_minutes(snapshot.measured_at_epoch, now_epoch);
    let fetch_age = last_successful_fetch_epoch.map(|t| now_epoch.saturating_sub(t).max(0) / 60);
    match (reading_age, fetch_age) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, _) => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertDecision {
    pub kind: AlertKind,
    pub title: &'static str,
    pub body: String,
    pub critical: bool,
}

/// Formats the human-readable notification body for an [`AlertKind`].
pub fn format_alert_body(
    settings: &AppSettings,
    snapshot: &GlucoseSnapshot,
    kind: AlertKind,
    age_minutes: Option<i64>,
) -> String {
    let unit = &settings.result_unit;
    let val_str = format_glucose_value(snapshot.value_mg_dl, unit);
    let arrow = trend_arrow_symbol(snapshot.trend_arrow);
    let low_eff = settings.effective_target_low(snapshot.target_low_mg_dl);
    let high_eff = settings.effective_target_high(snapshot.target_high_mg_dl);

    match kind {
        AlertKind::Stale => format!(
            "Last reading was {} ({} {}). Check the sensor and phone connection.",
            format_reading_age(age_minutes),
            val_str,
            unit
        ),
        AlertKind::UrgentLow => format!(
            "Blood glucose is {} {} {} — below urgent low ({} {})!",
            val_str,
            unit,
            arrow,
            format_glucose_value(settings.effective_urgent_low(), unit),
            unit
        ),
        AlertKind::Low | AlertKind::High => format!(
            "Blood glucose is {} {} {} (Target: {}–{} {})",
            val_str,
            unit,
            arrow,
            format_glucose_value(low_eff, unit),
            format_glucose_value(high_eff, unit),
            unit
        ),
    }
}

/// Evaluates whether an alert should fire for `snapshot` at `now_epoch`.
/// `last_successful_fetch_epoch` lets a failed/unreachable API surface as a stale alert even
/// when the last reading still looks recent.
/// Returns `(alert_to_fire, should_clear_last_alert)`.
pub fn evaluate_alert_decision(
    settings: &AppSettings,
    snapshot: &GlucoseSnapshot,
    last_fired: Option<(AlertKind, i64)>,
    now_epoch: i64,
    last_successful_fetch_epoch: Option<i64>,
) -> (Option<AlertDecision>, bool) {
    let age = effective_reading_age_minutes(snapshot, now_epoch, last_successful_fetch_epoch);
    let stale = snapshot.value_mg_dl > 0.0 && settings.is_reading_stale(age);

    let Some(kind) = settings.classify_alert(
        snapshot.value_mg_dl,
        snapshot.target_low_mg_dl,
        snapshot.target_high_mg_dl,
        stale,
    ) else {
        return (None, last_fired.is_some());
    };

    if !settings.should_fire_alert(kind, last_fired, now_epoch) {
        return (None, false);
    }

    let body = format_alert_body(settings, snapshot, kind, age);
    (
        Some(AlertDecision {
            kind,
            title: kind.title(),
            body,
            critical: kind.is_critical(),
        }),
        false,
    )
}

/// Outcome of applying Home Assistant `/api/states` synchronization data to local
/// medications and cross-device snooze state.
#[derive(Debug, Clone, Default)]
pub struct HaSyncOutcome {
    pub discovered_patients: Vec<HaPatientOption>,
    pub meds_merged: bool,
    pub should_push_meds: bool,
    /// `(effective_snooze_until_epoch, remote_updated_at_epoch)` when local snooze changed.
    pub updated_snooze: Option<(i64, i64)>,
}

/// Applies cross-device Home Assistant synchronization (`sensor.librelinkup_medications` and
/// `sensor.librelinkup_alert_snooze`) from a polled `/api/states` slice in a single shared path.
pub fn apply_ha_sync_from_states(
    states_arr: &[serde_json::Value],
    medication_store: &mut MedicationStore,
    sync_meds: bool,
    sync_snooze: bool,
    local_snooze_until: i64,
    local_snooze_updated_at: i64,
    now_epoch: i64,
) -> HaSyncOutcome {
    let mut out = HaSyncOutcome {
        discovered_patients: discover_ha_patients(states_arr),
        ..Default::default()
    };

    if sync_meds {
        let remote_recent_len = states_arr
            .iter()
            .find(|e| {
                e.get("entity_id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| id == "sensor.librelinkup_medications")
            })
            .and_then(|e| e.get("attributes"))
            .and_then(|a| a.get("recent_doses"))
            .and_then(|r| r.as_array())
            .map(Vec::len);

        if medication_store.merge_from_ha_states(states_arr) {
            out.meds_merged = true;
            out.should_push_meds = true;
        } else if !medication_store.medications.is_empty()
            && remote_recent_len != Some(medication_store.dose_logs.len().min(25))
        {
            out.should_push_meds = true;
        }
    }

    if sync_snooze
        && let Some((remote_until, remote_updated_at)) =
            parse_ha_remote_snooze(states_arr, local_snooze_updated_at)
    {
        let effective_until = if remote_until > now_epoch {
            remote_until
        } else {
            0
        };
        if effective_until != local_snooze_until {
            out.updated_snooze = Some((effective_until, remote_updated_at));
        }
    }

    out
}

/// Combined alert evaluation output covering scheduled medications, glucose threshold alerts,
/// and sensor expiration warnings.
#[derive(Debug, Clone, Default)]
pub struct FullAlertCycleResult {
    pub due_medication_reminders: Vec<MedicationReminder>,
    pub meds_slots_recorded: bool,
    pub glucose_alert: Option<AlertDecision>,
    pub clear_last_glucose_alert: bool,
    pub sensor_expiry_message: Option<String>,
}

/// Evaluates due medication reminders, glucose threshold/staleness alerts, and sensor expiration
/// countdown warnings in one platform-independent pass.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_full_alert_cycle(
    settings: &AppSettings,
    snapshot: &GlucoseSnapshot,
    medication_store: &mut MedicationStore,
    medication_last_notified: &mut HashMap<String, i64>,
    last_glucose_alert: Option<(AlertKind, i64)>,
    last_sensor_expiry_alert_epoch: i64,
    last_successful_fetch_epoch: Option<i64>,
    logged_in: bool,
    demo: bool,
    now_epoch: i64,
) -> FullAlertCycleResult {
    let mut out = FullAlertCycleResult::default();

    if !medication_store.medications.is_empty() {
        let due = medication_store.evaluate_due_reminders(now_epoch, medication_last_notified);
        if !due.is_empty() {
            medication_store.record_notified_slots(now_epoch, medication_last_notified);
            out.meds_slots_recorded = true;
            out.due_medication_reminders = due;
        }
    }

    if !logged_in {
        return out;
    }

    let (decision, clear_last) = evaluate_alert_decision(
        settings,
        snapshot,
        last_glucose_alert,
        now_epoch,
        last_successful_fetch_epoch,
    );
    out.glucose_alert = decision;
    out.clear_last_glucose_alert = clear_last;

    if !demo
        && !settings.is_snoozed(now_epoch)
        && snapshot.is_sensor_expiring_soon(now_epoch, settings.sensor_expiry_alert_hours)
        && now_epoch.saturating_sub(last_sensor_expiry_alert_epoch) >= 6 * 3600
    {
        out.sensor_expiry_message = Some(snapshot.format_sensor_status(now_epoch));
    }

    out
}

/// Formats the active snooze expiration timestamp (`"Snoozed until 3:45 PM"`), or returns
/// an empty string when alerts are not currently snoozed.
pub fn format_snooze_until(snooze_until_epoch: i64, now_epoch: i64) -> String {
    if snooze_until_epoch <= now_epoch {
        return String::new();
    }
    Local
        .timestamp_opt(snooze_until_epoch, 0)
        .single()
        .map(|dt| format!("Snoozed until {}", dt.format("%-I:%M %p")))
        .unwrap_or_else(|| "Snoozed".to_string())
}

/// Generates a deterministic 12-hour sample [`GlucoseSnapshot`] for offline demo / UI testing.
pub fn build_demo_snapshot(now_epoch: i64) -> GlucoseSnapshot {
    let now = chrono::DateTime::from_timestamp(now_epoch, 0)
        .map(|dt| dt.with_timezone(&Local))
        .unwrap_or_else(Local::now);
    let sample_values: [f64; 24] = [
        98.0, 102.0, 108.0, 115.0, 128.0, 146.0, 165.0, 178.0, 162.0, 148.0, 134.0, 122.0, 114.0,
        106.0, 99.0, 94.0, 91.0, 96.0, 104.0, 112.0, 119.0, 124.0, 116.0, 112.0,
    ];
    let graph_points = sample_values
        .iter()
        .enumerate()
        .map(|(idx, &val)| {
            let mins_ago = ((sample_values.len() - 1 - idx) * 15) as i64;
            let ts = (now - ChronoDuration::minutes(mins_ago))
                .format("%m/%d/%Y %-I:%M:%S %p")
                .to_string();
            GraphPoint {
                timestamp: ts,
                value_mg_dl: val,
            }
        })
        .collect();

    let activated = now_epoch - 2 * 86_400 - 6 * 3600;
    GlucoseSnapshot {
        value_mg_dl: 112.0,
        trend_arrow: 3,
        timestamp: now.format("%m/%d/%Y %-I:%M:%S %p").to_string(),
        target_low_mg_dl: DEFAULT_TARGET_LOW_MG_DL,
        target_high_mg_dl: DEFAULT_TARGET_HIGH_MG_DL,
        patient_id: "demo-patient-001".to_string(),
        first_name: "Sample".to_string(),
        last_name: "Sensor".to_string(),
        graph_points,
        measured_at_epoch: Some(now_epoch),
        sensor_serial: "40TUAM8CTE0".to_string(),
        sensor_activated_at_epoch: Some(activated),
        sensor_expires_at_epoch: Some(activated + 15 * 86_400),
        sensor_lifespan_days: 15,
        sensor_plus_model: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_snapshot_and_alert_evaluation_work() {
        let now = 1_700_000_000;
        let mut snap = build_demo_snapshot(now);
        assert_eq!(snap.value_mg_dl, 112.0);
        assert_eq!(snap.graph_points.len(), 24);

        let settings = AppSettings::default();
        let (decision, clear) = evaluate_alert_decision(&settings, &snap, None, now, Some(now));
        assert!(decision.is_none());
        assert!(!clear);

        snap.value_mg_dl = 50.0;
        let (decision, _) = evaluate_alert_decision(&settings, &snap, None, now, Some(now));
        let d = decision.expect("urgent low alert");
        assert_eq!(d.kind, AlertKind::UrgentLow);
        assert!(d.critical);
        assert!(d.body.contains("50"));
    }

    #[test]
    fn unknown_reading_age_is_treated_as_stale() {
        let now = 1_700_000_000;
        let mut snap = build_demo_snapshot(now);
        snap.value_mg_dl = 118.0;
        // Timestamp could not be parsed -> no epoch. The value must NOT be presented as live.
        snap.measured_at_epoch = None;
        snap.timestamp = "not-a-timestamp".into();

        assert_eq!(effective_reading_age_minutes(&snap, now, None), None);
        let settings = AppSettings::default();
        let (decision, _) = evaluate_alert_decision(&settings, &snap, None, now, None);
        let d = decision.expect("an unparseable timestamp must raise a stale alert");
        assert_eq!(d.kind, AlertKind::Stale);
        assert!(!d.critical);
    }

    #[test]
    fn failed_fetches_age_the_reading_via_the_fetch_clock() {
        let now = 1_700_000_000;
        let settings = AppSettings::default();
        let mut snap = build_demo_snapshot(now);
        snap.value_mg_dl = 118.0;

        // 1. No parseable reading timestamp, even if the last fetch just happened or was 45 min ago:
        //    effective age remains `None` so staleness detection fails closed.
        snap.measured_at_epoch = None;
        snap.timestamp = String::new();
        assert_eq!(
            effective_reading_age_minutes(&snap, now, Some(now - 45 * 60)),
            None
        );
        assert_eq!(effective_reading_age_minutes(&snap, now, Some(now)), None);
        let (decision, _) =
            evaluate_alert_decision(&settings, &snap, None, now, Some(now - 45 * 60));
        let d = decision.expect("stale alert expected");
        assert_eq!(d.kind, AlertKind::Stale);

        // 2. Both clocks known: the more conservative (older) age wins.
        snap.measured_at_epoch = Some(now - 50 * 60);
        assert_eq!(
            effective_reading_age_minutes(&snap, now, Some(now - 10 * 60)),
            Some(50)
        );
        // 3. Failed fetches age a reading even when its own timestamp was 5 min ago.
        snap.measured_at_epoch = Some(now - 5 * 60);
        assert_eq!(
            effective_reading_age_minutes(&snap, now, Some(now - 30 * 60)),
            Some(30)
        );
        // 4. Dead sensor: reading is 60 min old while a fetch succeeded right now -> Stale.
        snap.measured_at_epoch = Some(now - 60 * 60);
        assert_eq!(
            effective_reading_age_minutes(&snap, now, Some(now)),
            Some(60)
        );
        let (decision, _) = evaluate_alert_decision(&settings, &snap, None, now, Some(now));
        let d = decision.expect("dead sensor stale alert expected");
        assert_eq!(d.kind, AlertKind::Stale);
    }

    #[test]
    fn reauth_backoff_escalates_and_resets() {
        reset_reauth_backoff();
        let t0 = 1_700_000_000;
        assert!(reauth_allowed(t0));

        let d1 = record_reauth_failure(t0);
        assert_eq!(d1, 120, "2^1 minutes after the first failure");
        assert!(!reauth_allowed(t0));
        assert!(!reauth_allowed(t0 + 119));
        assert!(reauth_allowed(t0 + 120));

        let t1 = t0 + 120;
        let d2 = record_reauth_failure(t1);
        assert_eq!(d2, 240);
        assert!(!reauth_allowed(t1 + 239));
        assert!(reauth_allowed(t1 + 240));

        // Backoff is capped at one hour.
        let mut t = t1;
        for _ in 0..10 {
            let d = record_reauth_failure(t);
            assert!(d <= 3600);
            t += d;
        }

        record_reauth_success();
        assert!(reauth_allowed(t));
        reset_reauth_backoff();
    }

    #[test]
    fn staleness_boundary_and_none_fetch_clock() {
        let now = 1_700_000_000i64;
        let settings = AppSettings {
            stale_after_minutes: 20,
            alert_on_stale: true,
            ..Default::default()
        };
        let mut snap = build_demo_snapshot(now);
        snap.value_mg_dl = 115.0;

        // 19 minutes old with fetch right now -> fresh
        snap.measured_at_epoch = Some(now - 19 * 60);
        assert_eq!(
            effective_reading_age_minutes(&snap, now, Some(now)),
            Some(19)
        );
        let (d19, _) = evaluate_alert_decision(&settings, &snap, None, now, Some(now));
        assert!(d19.is_none());

        // Exactly 20 minutes old -> stale boundary triggers AlertKind::Stale
        snap.measured_at_epoch = Some(now - 20 * 60);
        assert_eq!(
            effective_reading_age_minutes(&snap, now, Some(now)),
            Some(20)
        );
        let (d20, _) = evaluate_alert_decision(&settings, &snap, None, now, Some(now));
        assert_eq!(d20.expect("stale at threshold").kind, AlertKind::Stale);

        // Fresh reading timestamp + last_fetch == None -> relies on reading clock (still 19 min)
        snap.measured_at_epoch = Some(now - 19 * 60);
        assert_eq!(effective_reading_age_minutes(&snap, now, None), Some(19));

        // Reading measured 25 min ago + last_fetch == None -> stale
        snap.measured_at_epoch = Some(now - 25 * 60);
        let (d_none_fetch, _) = evaluate_alert_decision(&settings, &snap, None, now, None);
        assert_eq!(
            d_none_fetch.expect("stale with None fetch").kind,
            AlertKind::Stale
        );
    }

    #[test]
    fn apply_ha_sync_from_states_merges_meds_and_resolves_snooze_timestamps() {
        let now = 1_700_000_000i64;
        let mut meds = MedicationStore::default();
        let med_id = meds
            .upsert_medication(
                "m1",
                "Metformin",
                "500 mg",
                crate::medications::MedicationSchedule::Daily,
                "08:00",
                0,
                true,
                "",
            )
            .unwrap();
        meds.log_dose(&med_id, None, now - 600, Some(110.0), "")
            .unwrap();

        let states = vec![
            serde_json::json!({
                "entity_id": "sensor.patient_measurement",
                "state": "114",
                "attributes": {
                    "unit_of_measurement": "mg/dL",
                    "device_class": "blood_glucose_concentration",
                    "friendly_name": "Patient Measurement"
                }
            }),
            serde_json::json!({
                "entity_id": "sensor.librelinkup_medications",
                "state": "1",
                "attributes": { "recent_doses": [] }
            }),
            serde_json::json!({
                "entity_id": "sensor.librelinkup_alert_snooze",
                "state": "snoozed",
                "attributes": {
                    "updated_at_epoch": now - 10,
                    "snooze_until_epoch": now + 900
                }
            }),
        ];

        // Newer remote snooze wins over older local snooze updated_at
        let out_newer = apply_ha_sync_from_states(&states, &mut meds, true, true, 0, now - 60, now);
        assert_eq!(out_newer.discovered_patients.len(), 1);
        assert!(
            out_newer.should_push_meds,
            "mismatch between remote empty recent_doses and local 1 dose"
        );
        assert_eq!(out_newer.updated_snooze, Some((now + 900, now - 10)));

        // Stale remote snooze is ignored when local snooze updated_at is newer
        let out_stale = apply_ha_sync_from_states(&states, &mut meds, true, true, 0, now, now);
        assert!(out_stale.updated_snooze.is_none());
    }

    #[test]
    fn evaluate_full_alert_cycle_combines_meds_expiry_snooze_and_clear() {
        use chrono::{Local, Timelike};
        let now = Local::now().timestamp();
        let dt = Local.timestamp_opt(now - 120, 0).single().unwrap();
        let sched_time = format!("{:02}:{:02}", dt.hour(), dt.minute());

        let mut meds = MedicationStore::default();
        meds.upsert_medication(
            "",
            "Insulin",
            "10 u",
            crate::medications::MedicationSchedule::Daily,
            &sched_time,
            0,
            true,
            "",
        )
        .unwrap();
        let mut notified = HashMap::new();

        let settings = AppSettings {
            urgent_low_mg_dl: 55.0,
            snooze_until_epoch: now + 600,
            sensor_expiry_alert_hours: 24,
            ..Default::default()
        };
        let mut snap = build_demo_snapshot(now);
        snap.value_mg_dl = 50.0;
        snap.sensor_expires_at_epoch = Some(now + 2 * 3600);

        let r1 = evaluate_full_alert_cycle(
            &settings,
            &snap,
            &mut meds,
            &mut notified,
            Some((AlertKind::High, now - 300)),
            0,
            Some(now),
            true,
            false,
            now,
        );
        assert_eq!(r1.due_medication_reminders.len(), 1);
        assert!(r1.meds_slots_recorded);
        assert_eq!(
            r1.glucose_alert.as_ref().expect("urgent low").kind,
            AlertKind::UrgentLow
        );
        // Sensor expiry suppressed while snoozed
        assert!(r1.sensor_expiry_message.is_none());

        // Unsnoozed & in range -> sensor expiry fires and clears previous alert
        let mut unsnoozed = settings.clone();
        unsnoozed.snooze_until_epoch = 0;
        snap.value_mg_dl = 112.0;
        let r2 = evaluate_full_alert_cycle(
            &unsnoozed,
            &snap,
            &mut meds,
            &mut notified,
            Some((AlertKind::UrgentLow, now - 60)),
            0,
            Some(now),
            true,
            false,
            now + 60,
        );
        assert!(r2.clear_last_glucose_alert);
        assert!(r2.sensor_expiry_message.is_some());

        // Within 6h cooldown -> sensor expiry does not refire
        let r3 = evaluate_full_alert_cycle(
            &unsnoozed,
            &snap,
            &mut meds,
            &mut notified,
            None,
            now + 60,
            Some(now),
            true,
            false,
            now + 120,
        );
        assert!(r3.sensor_expiry_message.is_none());
    }
}
