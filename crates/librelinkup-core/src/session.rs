//! Shared CGM session polling, automatic token re-authentication, alert evaluation,
//! snooze formatting, and demo telemetry generation used by both KDE Plasma and Windows.

use chrono::{Duration as ChronoDuration, Local, TimeZone};

use crate::api::{authenticate_async, fetch_cgm_snapshot_async, ApiError};
use crate::crypto::StoredCredentials;
use crate::event_log::log_info;
use crate::models::{
    format_glucose_value, format_reading_age, reading_age_minutes, trend_arrow_symbol, AlertKind,
    AppSettings, GlucoseSnapshot, GraphPoint, DEFAULT_TARGET_HIGH_MG_DL, DEFAULT_TARGET_LOW_MG_DL,
};

#[derive(Debug, Clone)]
pub struct RefreshOutcome {
    pub snapshot: GlucoseSnapshot,
    /// Present when credentials were updated (e.g. token refreshed after 401 or patient metadata populated).
    pub updated_credentials: Option<StoredCredentials>,
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
        log_info(
            "AUTH",
            "Bearer token expired (401); re-authenticating with stored credentials...",
        );
        if let Ok(reauth) = authenticate_async(
            &active_creds.country,
            &active_creds.username,
            &active_creds.password,
        )
        .await
        {
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
    })
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
/// Returns `(alert_to_fire, should_clear_last_alert)`.
pub fn evaluate_alert_decision(
    settings: &AppSettings,
    snapshot: &GlucoseSnapshot,
    last_fired: Option<(AlertKind, i64)>,
    now_epoch: i64,
) -> (Option<AlertDecision>, bool) {
    let age = reading_age_minutes(snapshot.measured_at_epoch, now_epoch);
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
            critical: kind != AlertKind::High,
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
    let now = Local::now();
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
        let (decision, clear) = evaluate_alert_decision(&settings, &snap, None, now);
        assert!(decision.is_none());
        assert!(!clear);

        snap.value_mg_dl = 50.0;
        let (decision, _) = evaluate_alert_decision(&settings, &snap, None, now);
        let d = decision.expect("urgent low alert");
        assert_eq!(d.kind, AlertKind::UrgentLow);
        assert!(d.critical);
        assert!(d.body.contains("50"));
    }
}
