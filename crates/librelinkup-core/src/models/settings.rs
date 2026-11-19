use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{
    clamp_ui_zoom_percent, format_glucose_value, format_target_range, trend_arrow_description,
    trend_arrow_symbol, GlucoseRangeStatus, GlucoseSnapshot, DEFAULT_ALERT_REPEAT_MINUTES,
    DEFAULT_STALE_AFTER_MINUTES, DEFAULT_TARGET_HIGH_MG_DL, DEFAULT_TARGET_LOW_MG_DL,
    DEFAULT_URGENT_LOW_MG_DL, MAX_POLL_INTERVAL_SECONDS, MIN_POLL_INTERVAL_SECONDS,
};
use crate::paths::{settings_file_path, write_file_atomic_0600};

pub fn clamp_poll_interval(seconds: u32) -> u32 {
    seconds.clamp(MIN_POLL_INTERVAL_SECONDS, MAX_POLL_INTERVAL_SECONDS)
}

/// Adds up to +/-10% jitter to a poll interval so multiple instances (or a restart) do not
/// hit the LibreView API in lockstep and trip its rate limiter.
pub fn jittered_poll_interval(seconds: u32) -> u32 {
    let base = clamp_poll_interval(seconds) as u64;
    let span = base / 10;
    if span == 0 {
        return base as u32;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_nanos()))
        .unwrap_or(0);
    let offset = nanos % (span * 2 + 1);
    let jittered = (base + offset).saturating_sub(span);
    clamp_poll_interval(jittered.min(u64::from(u32::MAX)) as u32)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    pub theme_mode: String,
    pub result_unit: String,
    pub country: String,
    pub language: String,
    pub window_mode: String,
    pub overlay_visible: bool,
    pub overlay_only_mode: bool,
    pub overlay_on_all_desktops: bool,
    pub overlay_dark_theme: bool,
    pub overlay_opacity_percent: i32,
    pub overlay_screen_name: String,
    pub overlay_x: i32,
    pub overlay_y: i32,
    pub overlay_width: i32,
    pub overlay_height: i32,
    pub window_screen_name: String,
    pub window_x: i32,
    pub window_y: i32,
    pub window_width: i32,
    pub window_height: i32,
    pub show_glucose_in_tray_icon: bool,
    pub show_glucose_in_tray_tooltip: bool,
    /// On KDE Plasma 6 (StatusNotifierItem), mark the tray icon `Passive` while blood glucose
    /// is In Range so it hides into the expandable System Tray drawer (`▲`) when configured as
    /// "Shown when relevant", and automatically promote it to `Active` / `NeedsAttention`
    /// whenever glucose goes Out of Range or Stale.
    pub tray_only_relevant_out_of_range: bool,
    pub tray_icon_theme: String,
    pub autostart_enabled: bool,
    pub start_minimized: bool,
    pub poll_interval_seconds: u32,
    pub bring_to_front_enabled: bool,
    pub flash_window_enabled: bool,
    pub audio_alert_enabled: bool,
    pub use_custom_sound: bool,
    pub custom_sound_path: String,
    pub override_threshold: bool,
    pub custom_target_low: f64,
    pub custom_target_high: f64,
    pub ui_text_scale: i32,
    /// Optional high-contrast accessibility mode (thicker borders, stronger focus rings, and
    /// explicit status icon prefixes alongside color coding).
    pub high_contrast_mode: bool,
    /// Urgent-low threshold (mg/dL). Urgent-low alerts ignore snooze and repeat every 5 min.
    pub urgent_low_mg_dl: f64,
    /// Minimum minutes between repeated alerts of the same kind.
    pub alert_repeat_minutes: u32,
    /// A reading older than this many minutes is considered stale (signal loss).
    pub stale_after_minutes: u32,
    /// Alert when readings become stale.
    pub alert_on_stale: bool,
    /// Non-urgent alerts are silenced until this Unix epoch second (0 = not snoozed).
    pub snooze_until_epoch: i64,
    /// Unix epoch second when `snooze_until_epoch` was last updated locally or via Home Assistant.
    pub snooze_updated_at_epoch: i64,
    /// Check GitHub Releases for a newer version automatically at startup.
    pub check_updates_on_startup: bool,
    /// Opt-in: also persist the account password inside the encrypted vault so expired
    /// bearer tokens can be silently refreshed without re-prompting. Defaults to `false`.
    pub remember_password: bool,
    /// Selected glycemic statistics window (`12h`, `7d`, `14d`, `30d`, `90d`).
    pub stats_window: String,
    /// Active telemetry data source: `"librelinkup"` (default) or `"home_assistant"`.
    pub data_source: String,
    /// Base URL for the Home Assistant instance (e.g. `"http://homeassistant.local:8123"`).
    pub ha_url: String,
    /// Optional explicit Home Assistant glucose sensor entity ID (e.g.
    /// `"sensor.patient_glucose_measurement"`). Empty = auto-discover LibreLink sensor.
    pub ha_sensor_entity_id: String,
    /// Two-way synchronization of medications and dose logs with Home Assistant
    /// (`sensor.librelinkup_medications` and `librelinkup_medication_dose_logged` event).
    pub ha_sync_medications: bool,
    /// Cross-device synchronization of alert snooze state with Home Assistant
    /// (`sensor.librelinkup_alert_snooze` and `librelinkup_alert_snooze_changed` event).
    pub ha_sync_snooze: bool,
    /// Warn when the active CGM sensor has fewer than this many hours remaining (`0` = disabled).
    pub sensor_expiry_alert_hours: i32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: "system".to_string(),
            result_unit: "mg/dL".to_string(),
            country: "us".to_string(),
            language: "en".to_string(),
            window_mode: "windowed".to_string(),
            overlay_visible: false,
            overlay_only_mode: false,
            overlay_on_all_desktops: true,
            overlay_dark_theme: true,
            overlay_opacity_percent: 88,
            overlay_screen_name: String::new(),
            overlay_x: -1,
            overlay_y: -1,
            overlay_width: 276,
            overlay_height: 88,
            window_screen_name: String::new(),
            window_x: -1,
            window_y: -1,
            window_width: 980,
            window_height: 680,
            show_glucose_in_tray_icon: true,
            show_glucose_in_tray_tooltip: true,
            tray_only_relevant_out_of_range: false,
            tray_icon_theme: "auto".to_string(),
            autostart_enabled: true,
            start_minimized: true,
            poll_interval_seconds: MIN_POLL_INTERVAL_SECONDS,
            bring_to_front_enabled: false,
            flash_window_enabled: true,
            audio_alert_enabled: true,
            use_custom_sound: false,
            custom_sound_path: String::new(),
            override_threshold: false,
            custom_target_low: DEFAULT_TARGET_LOW_MG_DL,
            custom_target_high: DEFAULT_TARGET_HIGH_MG_DL,
            ui_text_scale: 100,
            high_contrast_mode: false,
            urgent_low_mg_dl: DEFAULT_URGENT_LOW_MG_DL,
            alert_repeat_minutes: DEFAULT_ALERT_REPEAT_MINUTES,
            stale_after_minutes: DEFAULT_STALE_AFTER_MINUTES,
            alert_on_stale: true,
            snooze_until_epoch: 0,
            snooze_updated_at_epoch: 0,
            check_updates_on_startup: true,
            remember_password: false,
            stats_window: "14d".to_string(),
            data_source: "librelinkup".to_string(),
            ha_url: String::new(),
            ha_sensor_entity_id: String::new(),
            ha_sync_medications: true,
            ha_sync_snooze: true,
            sensor_expiry_alert_hours: 24,
        }
    }
}

impl AppSettings {
    pub fn effective_target_low(&self, api_target_low: f64) -> f64 {
        if self.override_threshold && self.custom_target_low > 0.0 {
            self.custom_target_low
        } else if api_target_low > 0.0 {
            api_target_low
        } else {
            DEFAULT_TARGET_LOW_MG_DL
        }
    }

    pub fn effective_target_high(&self, api_target_high: f64) -> f64 {
        if self.override_threshold && self.custom_target_high > 0.0 {
            self.custom_target_high
        } else if api_target_high > 0.0 {
            api_target_high
        } else {
            DEFAULT_TARGET_HIGH_MG_DL
        }
    }

    pub fn should_trigger_alert(&self, value_mg_dl: f64, api_low: f64, api_high: f64) -> bool {
        if !value_mg_dl.is_finite() || value_mg_dl <= 0.0 {
            return false;
        }
        let low = self.effective_target_low(api_low);
        let high = self.effective_target_high(api_high);
        value_mg_dl < low || value_mg_dl > high
    }

    /// Returns whether the KDE Plasma System Tray StatusNotifierItem should be marked
    /// `Active` ("relevant", visible on the panel tray bar) vs. `Passive` (hidden inside the
    /// expandable tray drawer when the tray entry is set to "Shown when relevant").
    ///
    /// - When `tray_only_relevant_out_of_range` is `false` (default), the tray icon is always
    ///   `Active` (`true`).
    /// - When `tray_only_relevant_out_of_range` is `true`, the tray icon is `Passive` (`false`)
    ///   while blood glucose is fresh and within target range (`InRange`), and automatically
    ///   becomes `Active` (`true`) when glucose goes below `target_low`, above `target_high`,
    ///   or when the reading becomes stale.
    pub fn is_tray_relevant(&self, snapshot: &GlucoseSnapshot, reading_stale: bool) -> bool {
        if !self.tray_only_relevant_out_of_range {
            return true;
        }
        if !snapshot.value_mg_dl.is_finite() || snapshot.value_mg_dl <= 0.0 {
            return false;
        }
        if reading_stale {
            return true;
        }
        self.should_trigger_alert(
            snapshot.value_mg_dl,
            snapshot.target_low_mg_dl,
            snapshot.target_high_mg_dl,
        )
    }

    /// Returns `true` when the reading is fresh and below the urgent-low threshold, so the
    /// StatusNotifierItem can request `NeedsAttention` status on KDE Plasma.
    pub fn is_tray_urgent_attention(&self, snapshot: &GlucoseSnapshot, reading_stale: bool) -> bool {
        !reading_stale
            && snapshot.value_mg_dl.is_finite()
            && snapshot.value_mg_dl > 0.0
            && snapshot.value_mg_dl < self.effective_urgent_low()
    }

    pub fn format_tray_icon_badge(&self, snapshot: &GlucoseSnapshot) -> String {
        if !self.show_glucose_in_tray_icon || snapshot.value_mg_dl <= 0.0 {
            return String::new();
        }
        format_glucose_value(snapshot.value_mg_dl, &self.result_unit)
    }

    pub fn format_tray_hover_tooltip(&self, snapshot: &GlucoseSnapshot) -> String {
        if !self.show_glucose_in_tray_tooltip {
            return "LibreLinkUp Rust Desktop".to_string();
        }
        if snapshot.value_mg_dl <= 0.0 {
            return "LibreLinkUp Rust Desktop — Waiting for glucose reading...".to_string();
        }
        let val_str = format_glucose_value(snapshot.value_mg_dl, &self.result_unit);
        let arrow = trend_arrow_symbol(snapshot.trend_arrow);
        let trend_desc = trend_arrow_description(snapshot.trend_arrow);
        let low = self.effective_target_low(snapshot.target_low_mg_dl);
        let high = self.effective_target_high(snapshot.target_high_mg_dl);
        let status = GlucoseRangeStatus::classify(snapshot.value_mg_dl, low, high);
        let range_str = format_target_range(low, high, &self.result_unit);
        let patient = format!("{} {}", snapshot.first_name.trim(), snapshot.last_name.trim())
            .trim()
            .to_string();

        let mut lines = vec![
            format!(
                "Blood Sugar: {} {} {} ({} • {})",
                val_str,
                self.result_unit,
                arrow,
                trend_desc,
                status.label()
            ),
            format!("Target Range: {range_str}"),
        ];
        if !patient.is_empty() {
            lines.push(format!("Patient: {patient}"));
        }
        if !snapshot.timestamp.is_empty() {
            lines.push(format!("Measured: {}", snapshot.timestamp));
        }
        lines.join("\n")
    }
}

pub fn load_settings_from_path(path: &Path) -> AppSettings {
    if let Ok(raw) = fs::read_to_string(path)
        && let Ok(mut parsed) = serde_json::from_str::<AppSettings>(&raw) {
            parsed.poll_interval_seconds = clamp_poll_interval(parsed.poll_interval_seconds);
            parsed.ui_text_scale = clamp_ui_zoom_percent(parsed.ui_text_scale);
            return parsed;
        }
    AppSettings::default()
}

pub fn save_settings_to_path(settings: &AppSettings, path: &Path) -> Result<(), String> {
    let mut normalized = settings.clone();
    normalized.poll_interval_seconds = clamp_poll_interval(normalized.poll_interval_seconds);
    normalized.ui_text_scale = clamp_ui_zoom_percent(normalized.ui_text_scale);
    let json = serde_json::to_string_pretty(&normalized)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    write_file_atomic_0600(path, json.as_bytes())
        .map_err(|e| format!("Failed to write settings file: {e}"))
}

pub fn load_default_settings() -> AppSettings {
    load_settings_from_path(&settings_file_path())
}

pub fn save_default_settings(settings: &AppSettings) -> Result<(), String> {
    save_settings_to_path(settings, &settings_file_path())
}
