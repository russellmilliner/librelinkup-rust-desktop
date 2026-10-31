//! Pure(ish) state transitions shared by the three controllers.
//!
//! Nothing here touches Qt types: it mutates [`AppState`] only. The controllers are thin
//! property mirrors on top, which keeps the Qt bridge boring and the logic unit-testable.

use crate::analytics::{save_default_glucose_history, StatsWindow, HISTORY_PERSIST_MIN_INTERVAL_SECS};
use crate::bridge::state::AppState;
use crate::event_log::{get_event_count, get_events_json};
use crate::models::{
    clamp_poll_interval, format_glucose_value, format_reading_age, format_target_range,
    trend_arrow_description, trend_arrow_symbol, AppSettings, GlucoseRangeStatus, STALE_COLOR_HEX,
};
use crate::session::{effective_reading_age_minutes, format_snooze_until};

/// Builds the persisted settings from the live state.
///
/// Fields the KDE front-end does not own (`ui_text_scale`, `tray_icon_theme`, and any field
/// added in a future version) are read back from disk and preserved, so toggling one setting
/// can never silently reset another.
pub fn current_settings(state: &AppState) -> AppSettings {
    let mut s = crate::models::load_default_settings();
    s.theme_mode = state.theme_mode.clone();
    s.ui_text_scale = crate::models::clamp_ui_zoom_percent(state.ui_zoom_percent);
    s.high_contrast_mode = state.high_contrast_mode;
    s.result_unit = state.result_unit.clone();
    s.country = state.country.clone();
    s.language = state.language.clone();
    s.window_mode = state.window_mode.clone();
    s.overlay_visible = state.overlay_visible;
    s.overlay_only_mode = state.overlay_only_mode;
    s.overlay_on_all_desktops = state.overlay_on_all_desktops;
    s.overlay_dark_theme = state.overlay_dark_theme;
    s.overlay_opacity_percent = state.overlay_opacity_percent.clamp(20, 100);
    s.overlay_screen_name = state.overlay_screen_name.clone();
    s.overlay_x = state.overlay_x;
    s.overlay_y = state.overlay_y;
    s.overlay_width = state.overlay_width.clamp(230, 600);
    s.overlay_height = state.overlay_height.clamp(80, 260);
    s.window_screen_name = state.window_screen_name.clone();
    s.window_x = state.window_x;
    s.window_y = state.window_y;
    s.window_width = state.window_width.clamp(780, 3840);
    s.window_height = state.window_height.clamp(540, 2160);
    s.show_glucose_in_tray_icon = state.show_glucose_in_tray_icon;
    s.show_glucose_in_tray_tooltip = state.show_glucose_in_tray_tooltip;
    s.autostart_enabled = state.autostart_enabled;
    s.start_minimized = state.start_minimized;
    s.poll_interval_seconds = clamp_poll_interval(state.poll_interval_seconds.max(0) as u32);
    s.bring_to_front_enabled = state.bring_to_front_enabled;
    s.flash_window_enabled = state.flash_window_enabled;
    s.audio_alert_enabled = state.audio_alert_enabled;
    s.use_custom_sound = state.use_custom_sound;
    s.custom_sound_path = state.custom_sound_path.clone();
    s.override_threshold = state.override_threshold;
    s.custom_target_low = state.custom_target_low;
    s.custom_target_high = state.custom_target_high;
    s.urgent_low_mg_dl = state.urgent_low_mg_dl;
    s.alert_repeat_minutes = state.alert_repeat_minutes.clamp(1, 240) as u32;
    s.stale_after_minutes = state.stale_after_minutes.clamp(5, 120) as u32;
    s.alert_on_stale = state.alert_on_stale;
    s.snooze_until_epoch = state.snooze_until_epoch;
    s.snooze_updated_at_epoch = state.snooze_updated_at_epoch;
    s.check_updates_on_startup = state.check_updates_on_startup;
    s.remember_password = state.remember_password;
    s.stats_window = StatsWindow::from_code(&state.stats_window).code().to_string();
    s.data_source = state.data_source.clone();
    s.ha_url = state.ha_url.clone();
    s.ha_sensor_entity_id = state.ha_sensor_entity_id.clone();
    s.ha_sync_medications = state.ha_sync_medications;
    s.ha_sync_snooze = state.ha_sync_snooze;
    s.sensor_expiry_alert_hours = state.sensor_expiry_alert_hours.clamp(0, 168);
    s
}

/// Persists the live state's settings to disk.
pub fn persist_settings(state: &AppState) -> Result<(), String> {
    crate::models::save_default_settings(&current_settings(state))
}

/// Recomputes every snapshot-derived string: glucose display, trend, range status, targets,
/// reading age, staleness and the snooze banner.
pub fn refresh_display(state: &mut AppState) {
    let settings = current_settings(state);
    let snapshot = state.snapshot.clone();
    let now = crate::bridge::state::now_epoch();

    let eff_low = settings.effective_target_low(snapshot.target_low_mg_dl);
    let eff_high = settings.effective_target_high(snapshot.target_high_mg_dl);
    let val_display = format_glucose_value(snapshot.value_mg_dl, &settings.result_unit);

    let age_minutes =
        effective_reading_age_minutes(&snapshot, now, state.last_successful_fetch_epoch);
    let age_text = format_reading_age(age_minutes);
    // Fail closed: an unknown age counts as stale (see `AppSettings::is_reading_stale`).
    let stale = snapshot.value_mg_dl > 0.0 && settings.is_reading_stale(age_minutes);

    let (status_label, color_hex) = if stale {
        ("Stale", STALE_COLOR_HEX)
    } else if snapshot.value_mg_dl > 0.0 {
        let st = GlucoseRangeStatus::classify(snapshot.value_mg_dl, eff_low, eff_high);
        (st.label(), st.color_hex())
    } else {
        ("Waiting", "#22c55e")
    };

    state.glucose_display = val_display;
    state.glucose_mg_dl = snapshot.value_mg_dl;
    state.result_unit = settings.result_unit.clone();
    state.trend_arrow = snapshot.trend_arrow;
    state.trend_symbol = trend_arrow_symbol(snapshot.trend_arrow).to_string();
    state.trend_description = trend_arrow_description(snapshot.trend_arrow).to_string();
    state.range_status_label = status_label.to_string();
    state.range_color_hex = color_hex.to_string();
    state.target_range_display = format_target_range(eff_low, eff_high, &settings.result_unit);
    state.target_low_mg_dl = eff_low;
    state.target_high_mg_dl = eff_high;
    state.reading_age_display = age_text.clone();
    state.reading_stale = stale;
    state.alerts_snoozed = settings.is_snoozed(now);
    state.snooze_until_display = format_snooze_until(settings.snooze_until_epoch, now);
    state.accessible_summary =
        crate::models::accessible_glucose_announcement(&snapshot, &settings, stale, &age_text);
    state.sensor_status_display = snapshot.format_sensor_status(now);
    state.sensor_serial = snapshot.sensor_serial.clone();
    state.sensor_in_warmup = snapshot.is_sensor_in_warmup(now);
    state.sensor_expiring_soon =
        snapshot.is_sensor_expiring_soon(now, state.sensor_expiry_alert_hours);

    if !snapshot.timestamp.is_empty() {
        state.last_updated = if age_text.is_empty() {
            snapshot.timestamp.clone()
        } else {
            format!("{} ({age_text})", snapshot.timestamp)
        };
    }
    let full_name = format!(
        "{} {}",
        snapshot.first_name.trim(),
        snapshot.last_name.trim()
    )
    .trim()
    .to_string();
    if !full_name.is_empty() {
        state.patient_name = full_name;
    }
    if !snapshot.patient_id.is_empty() {
        state.patient_id = snapshot.patient_id.clone();
    }
    state.graph_points_json =
        serde_json::to_string(&snapshot.graph_points).unwrap_or_else(|_| "[]".to_string());

    if state.logged_in {
        if stale {
            state.status_message = format!(
                "No new sensor readings ({age_text}) — check sensor/phone"
            );
        } else if state.status_message.starts_with("No new sensor readings") {
            state.status_message = "Live CGM telemetry active".to_string();
        }
    }
}

/// Recomputes eA1C / GMI / TIR for the selected window.
pub fn refresh_analytics(state: &mut AppState) {
    let settings = current_settings(state);
    let win = StatsWindow::from_code(&state.stats_window);
    let now = crate::bridge::state::now_epoch();
    let stats = state.glucose_history.compute_stats(
        win,
        now,
        settings.effective_target_low(state.snapshot.target_low_mg_dl),
        settings.effective_target_high(state.snapshot.target_high_mg_dl),
        Some(&state.snapshot),
    );
    state.ea1c_headline = stats.format_headline();
    state.ea1c_subline = stats.format_subline(&settings.result_unit);
    if stats.has_data() {
        state.ea1c_display = format!("{:.1}%", stats.ea1c_percent);
        state.gmi_display = format!("{:.1}%", stats.gmi_percent);
        state.tir_display = format!("{:.0}%", stats.tir_in_range_percent);
    } else {
        state.ea1c_display = "--".to_string();
        state.gmi_display = "--".to_string();
        state.tir_display = "--".to_string();
    }
}

/// Recomputes the medication list/dose/graph-marker JSON blobs consumed by QML.
pub fn refresh_medication_views(state: &mut AppState) {
    let now = crate::bridge::state::now_epoch();
    let items = state.medication_store.view_items(now);
    state.medications_json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string());
    state.medication_doses_json = serde_json::to_string(&state.medication_store.dose_logs)
        .unwrap_or_else(|_| "[]".to_string());
    let markers = state
        .medication_store
        .graph_markers_for_snapshot(&state.snapshot, now);
    state.graph_doses_json = serde_json::to_string(&markers).unwrap_or_else(|_| "[]".to_string());
    state.medications_summary = state.medication_store.summary_line(now);
    state.medications_count = items.len() as i32;
    state.medication_doses_count = state.medication_store.dose_logs.len() as i32;
}

/// Mirrors the in-memory event log into the state (used by the log viewer).
pub fn refresh_logs(state: &mut AppState) {
    state.event_log_json = get_events_json();
    state.event_log_count = get_event_count() as i32;
}

/// Persists the glucose history vault at most once per [`HISTORY_PERSIST_MIN_INTERVAL_SECS`]
/// (or immediately when `force` is set).
pub fn persist_history_if_due(state: &mut AppState, force: bool) {
    let now = crate::bridge::state::now_epoch();
    if !force && now - state.last_history_persist_epoch < HISTORY_PERSIST_MIN_INTERVAL_SECS {
        return;
    }
    match save_default_glucose_history(&state.glucose_history) {
        Ok(()) => state.last_history_persist_epoch = now,
        Err(e) => crate::event_log::log_error(
            "CRYPTO",
            format!("Failed to persist glucose history vault: {e}"),
        ),
    }
}

/// Ingests a fresh snapshot into the history store, persisting only when the data changed.
pub fn ingest_history(state: &mut AppState, snapshot: &crate::models::GlucoseSnapshot) {
    let now = crate::bridge::state::now_epoch();
    if state.glucose_history.ingest_snapshot(snapshot, now) {
        persist_history_if_due(state, false);
    }
}

/// Spawns a non-blocking background push of the current medication store (and optional logged
/// dose event) to Home Assistant when `ha_sync_medications` is enabled and Home Assistant
/// credentials are configured.
pub fn spawn_ha_medications_push(logged_dose: Option<crate::medications::MedicationDoseLog>) {
    let (enabled, ha_url, ha_token, store) = crate::bridge::state::with_state(|s| {
        let tok = s
            .creds
            .as_ref()
            .map(|c| c.ha_token.clone())
            .unwrap_or_default();
        (
            s.ha_sync_medications,
            s.ha_url.clone(),
            tok,
            s.medication_store.clone(),
        )
    });
    if !enabled || ha_url.trim().is_empty() || ha_token.trim().is_empty() {
        return;
    }
    if let Some(rt) = crate::bridge::autostart::background_runtime() {
        rt.spawn(async move {
            if let Err(e) = crate::api::publish_ha_medications_state_async(
                &ha_url,
                &ha_token,
                &store,
                logged_dose.as_ref(),
            )
            .await
            {
                crate::event_log::log_warn(
                    "MEDS",
                    format!("Failed to sync medications to Home Assistant: {e}"),
                );
            }
        });
    }
}

/// Spawns a non-blocking background push of the current alert snooze state to Home Assistant
/// when `ha_sync_snooze` is enabled and Home Assistant credentials are configured.
pub fn spawn_ha_snooze_push() {
    let (enabled, ha_url, ha_token, until, updated) = crate::bridge::state::with_state(|s| {
        let tok = s
            .creds
            .as_ref()
            .map(|c| c.ha_token.clone())
            .unwrap_or_default();
        (
            s.ha_sync_snooze,
            s.ha_url.clone(),
            tok,
            s.snooze_until_epoch,
            s.snooze_updated_at_epoch,
        )
    });
    if !enabled || ha_url.trim().is_empty() || ha_token.trim().is_empty() {
        return;
    }
    if let Some(rt) = crate::bridge::autostart::background_runtime() {
        rt.spawn(async move {
            if let Err(e) =
                crate::api::publish_ha_snooze_state_async(&ha_url, &ha_token, until, updated).await
            {
                crate::event_log::log_warn(
                    "ALERT",
                    format!("Failed to sync alert snooze to Home Assistant: {e}"),
                );
            }
        });
    }
}
