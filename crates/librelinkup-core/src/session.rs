//! Shared CGM session polling, automatic token re-authentication, alert evaluation,
//! snooze formatting, and demo telemetry generation used by both KDE Plasma and Windows.

use std::sync::{Mutex, OnceLock};

use chrono::{Duration as ChronoDuration, Local, TimeZone};

use crate::api::{
    authenticate_async, fetch_cgm_snapshot_async, fetch_ha_cgm_snapshot_and_states_async,
    fetch_ha_states_array_async, ApiError,
};
use crate::crypto::StoredCredentials;
use crate::event_log::{log_info, log_warn};
use crate::models::{
    format_glucose_value, format_reading_age, reading_age_minutes, trend_arrow_symbol, AlertKind,
    AppSettings, GlucoseSnapshot, GraphPoint, DEFAULT_TARGET_HIGH_MG_DL, DEFAULT_TARGET_LOW_MG_DL,
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

    let mut result = fetch_cgm_snapshot_async(
        &active_creds.country,
        &active_creds.token,
        &active_creds.account_id,
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
                Ok(reauth) => {
                    record_reauth_success();
                    active_creds.token = reauth.token;
                    active_creds.account_id = reauth.account_id;
                    active_creds.country = reauth.country;
                    reauthenticated = true;
                    result = fetch_cgm_snapshot_async(
                        &active_creds.country,
                        &active_creds.token,
                        &active_creds.account_id,
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
        if !ha_url.trim().is_empty() && !creds.ha_token.trim().is_empty() {
            outcome.ha_states = fetch_ha_states_array_async(ha_url, &creds.ha_token).await.ok();
        }
        Ok(outcome)
    }
}

/// Effective age of the displayed reading in whole minutes.
///
/// A reading can be old in two ways: its own timestamp is old, or we have not managed to
/// fetch anything recently (no signal, server outage, suspended laptop). Taking the minimum
/// of the two makes staleness detection fail closed - if we cannot confirm the data is
/// current, the reading is reported as stale.
pub fn effective_reading_age_minutes(
    snapshot: &GlucoseSnapshot,
    now_epoch: i64,
    last_successful_fetch_epoch: Option<i64>,
) -> Option<i64> {
    let reading_age = reading_age_minutes(snapshot.measured_at_epoch, now_epoch);
    let fetch_age = last_successful_fetch_epoch.map(|t| (now_epoch - t).max(0) / 60);
    match (reading_age, fetch_age) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, age) => age,
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

        // 1. No parseable reading timestamp, but the last successful fetch was 45 min ago:
        //    the fetch clock supplies the age so the value is not shown as live.
        snap.measured_at_epoch = None;
        snap.timestamp = String::new();
        assert_eq!(
            effective_reading_age_minutes(&snap, now, Some(now - 45 * 60)),
            Some(45)
        );
        let (decision, _) =
            evaluate_alert_decision(&settings, &snap, None, now, Some(now - 45 * 60));
        let d = decision.expect("stale alert expected");
        assert_eq!(d.kind, AlertKind::Stale);

        // 2. Both clocks known: the more conservative (older) age wins.
        snap.measured_at_epoch = Some(now - 50 * 60);
        assert_eq!(
            effective_reading_age_minutes(&snap, now, Some(now - 10 * 60)),
            Some(10)
        );
        // 3. A reading older than the last fetch keeps its own age.
        snap.measured_at_epoch = Some(now - 5 * 60);
        assert_eq!(
            effective_reading_age_minutes(&snap, now, Some(now - 30 * 60)),
            Some(5)
        );
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
}
