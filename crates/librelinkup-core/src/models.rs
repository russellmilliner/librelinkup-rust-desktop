use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::paths::{settings_file_path, write_file_atomic_0600};

pub const LOW_CRITICAL_MG_DL: f64 = 70.0;
pub const HIGH_CRITICAL_MG_DL: f64 = 240.0;
pub const DEFAULT_TARGET_LOW_MG_DL: f64 = 70.0;
pub const DEFAULT_TARGET_HIGH_MG_DL: f64 = 180.0;
pub const DEFAULT_URGENT_LOW_MG_DL: f64 = 55.0;
pub const DEFAULT_ALERT_REPEAT_MINUTES: u32 = 15;
pub const DEFAULT_STALE_AFTER_MINUTES: u32 = 15;
/// Urgent-low alerts repeat at least this often regardless of the configured repeat interval.
pub const URGENT_LOW_REPEAT_MINUTES: u32 = 5;
/// Minimum allowed polling interval in seconds (LibreView rate-limit floor).
pub const MIN_POLL_INTERVAL_SECONDS: u32 = 60;
/// Maximum allowed polling interval in seconds.
pub const MAX_POLL_INTERVAL_SECONDS: u32 = 3600;
/// Gray used for stale readings in the UI, tray badge and overlay.
pub const STALE_COLOR_HEX: &str = "#6b7280";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlucoseRangeStatus {
    CriticalLow,
    OutOfTarget,
    InRange,
    CriticalHigh,
}

impl GlucoseRangeStatus {
    pub fn classify(value_mg_dl: f64, target_low_mg_dl: f64, target_high_mg_dl: f64) -> Self {
        if value_mg_dl < LOW_CRITICAL_MG_DL {
            Self::CriticalLow
        } else if value_mg_dl > HIGH_CRITICAL_MG_DL {
            Self::CriticalHigh
        } else if value_mg_dl < target_low_mg_dl || value_mg_dl > target_high_mg_dl {
            Self::OutOfTarget
        } else {
            Self::InRange
        }
    }

    pub fn color_hex(&self) -> &'static str {
        match self {
            Self::CriticalLow => "#ef4444",  // Red
            Self::OutOfTarget => "#eab308",  // Amber/Yellow
            Self::InRange => "#22c55e",      // Green
            Self::CriticalHigh => "#f97316", // Orange
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::CriticalLow => "Low",
            Self::OutOfTarget => "Out of Target",
            Self::InRange => "In Range",
            Self::CriticalHigh => "High",
        }
    }
}

pub fn trend_arrow_symbol(trend: i32) -> &'static str {
    match trend {
        1 => "↓",
        2 => "↘",
        3 => "→",
        4 => "↗",
        5 => "↑",
        _ => "→",
    }
}

pub fn trend_arrow_description(trend: i32) -> &'static str {
    match trend {
        1 => "Falling Quickly",
        2 => "Falling",
        3 => "Stable",
        4 => "Rising",
        5 => "Rising Quickly",
        _ => "Stable",
    }
}

pub fn convert_mg_dl_to_unit(value_mg_dl: f64, unit: &str) -> f64 {
    if unit.eq_ignore_ascii_case("mmol/L") {
        ((value_mg_dl / 18.0) * 10.0).round() / 10.0
    } else {
        value_mg_dl.round()
    }
}

pub fn format_glucose_value(value_mg_dl: f64, unit: &str) -> String {
    if value_mg_dl <= 0.0 || value_mg_dl.is_nan() {
        return "--".to_string();
    }
    let converted = convert_mg_dl_to_unit(value_mg_dl, unit);
    if unit.eq_ignore_ascii_case("mmol/L") {
        let s = format!("{converted:.1}");
        if let Some(stripped) = s.strip_suffix(".0") {
            stripped.to_string()
        } else {
            s
        }
    } else {
        format!("{:.0}", converted)
    }
}

pub fn format_target_range(target_low_mg_dl: f64, target_high_mg_dl: f64, unit: &str) -> String {
    let low = format_glucose_value(target_low_mg_dl, unit);
    let high = format_glucose_value(target_high_mg_dl, unit);
    let unit_str = if unit.eq_ignore_ascii_case("mmol/L") {
        "mmol/L"
    } else {
        "mg/dL"
    };
    format!("{low}–{high} {unit_str}")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphPoint {
    pub timestamp: String,
    pub value_mg_dl: f64,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct GlucoseSnapshot {
    pub value_mg_dl: f64,
    pub trend_arrow: i32,
    pub timestamp: String,
    pub target_low_mg_dl: f64,
    pub target_high_mg_dl: f64,
    pub patient_id: String,
    pub first_name: String,
    pub last_name: String,
    pub graph_points: Vec<GraphPoint>,
    /// Unix epoch seconds when the current reading was measured (parsed from LibreView's
    /// `FactoryTimestamp`, which is UTC). `None` when unknown (e.g. sample data).
    #[serde(default)]
    pub measured_at_epoch: Option<i64>,
    /// Active CGM sensor serial number (e.g. `"40TUAM8CTE0"`).
    #[serde(default)]
    pub sensor_serial: String,
    /// Unix epoch seconds when the current CGM sensor was activated/applied.
    #[serde(default)]
    pub sensor_activated_at_epoch: Option<i64>,
    /// Unix epoch seconds when the current CGM sensor expires.
    #[serde(default)]
    pub sensor_expires_at_epoch: Option<i64>,
    /// Sensor rated lifespan in days (typically `14` or `15` for Plus sensors).
    #[serde(default)]
    pub sensor_lifespan_days: i32,
    /// Whether the active sensor is a Plus (15-day) model.
    #[serde(default)]
    pub sensor_plus_model: bool,
}

impl fmt::Debug for GlucoseSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GlucoseSnapshot")
            .field("value_mg_dl", &self.value_mg_dl)
            .field("trend_arrow", &self.trend_arrow)
            .field("timestamp", &self.timestamp)
            .field("target_low_mg_dl", &self.target_low_mg_dl)
            .field("target_high_mg_dl", &self.target_high_mg_dl)
            .field("patient_id", &"[REDACTED]")
            .field("first_name", &"[REDACTED]")
            .field("last_name", &"[REDACTED]")
            .field("graph_points_len", &self.graph_points.len())
            .field("measured_at_epoch", &self.measured_at_epoch)
            .field("sensor_serial", &"[REDACTED]")
            .field("sensor_activated_at_epoch", &self.sensor_activated_at_epoch)
            .field("sensor_expires_at_epoch", &self.sensor_expires_at_epoch)
            .field("sensor_lifespan_days", &self.sensor_lifespan_days)
            .field("sensor_plus_model", &self.sensor_plus_model)
            .finish()
    }
}

impl Default for GlucoseSnapshot {
    fn default() -> Self {
        Self {
            value_mg_dl: 0.0,
            trend_arrow: 3,
            timestamp: String::new(),
            target_low_mg_dl: DEFAULT_TARGET_LOW_MG_DL,
            target_high_mg_dl: DEFAULT_TARGET_HIGH_MG_DL,
            patient_id: String::new(),
            first_name: String::new(),
            last_name: String::new(),
            graph_points: Vec::new(),
            measured_at_epoch: None,
            sensor_serial: String::new(),
            sensor_activated_at_epoch: None,
            sensor_expires_at_epoch: None,
            sensor_lifespan_days: 0,
            sensor_plus_model: false,
        }
    }
}

/// FreeStyle Libre sensor warmup duration after initial activation (60 minutes).
pub const SENSOR_WARMUP_SECONDS: i64 = 60 * 60;

impl GlucoseSnapshot {
    /// Returns `true` when the sensor was activated within the last 60 minutes (`SENSOR_WARMUP_SECONDS`).
    pub fn is_sensor_in_warmup(&self, now_epoch: i64) -> bool {
        self.sensor_activated_at_epoch
            .is_some_and(|act| now_epoch >= act && (now_epoch - act) < SENSOR_WARMUP_SECONDS)
    }

    /// Remaining warmup minutes (`1..=60`) if the sensor is currently warming up.
    pub fn sensor_warmup_minutes_remaining(&self, now_epoch: i64) -> Option<i64> {
        let act = self.sensor_activated_at_epoch?;
        if now_epoch >= act && (now_epoch - act) < SENSOR_WARMUP_SECONDS {
            let rem_secs = (act + SENSOR_WARMUP_SECONDS - now_epoch).max(1);
            Some((rem_secs + 59) / 60)
        } else {
            None
        }
    }

    /// Effective Unix epoch seconds when the sensor expires, either from explicit
    /// `sensor_expires_at_epoch` or derived from `sensor_activated_at_epoch + lifespan_days`.
    pub fn effective_sensor_expires_at_epoch(&self) -> Option<i64> {
        self.sensor_expires_at_epoch.or_else(|| {
            self.sensor_activated_at_epoch.map(|act| {
                let days = if self.sensor_lifespan_days > 0 {
                    i64::from(self.sensor_lifespan_days)
                } else if self.sensor_plus_model {
                    15
                } else {
                    14
                };
                act + days * 86_400
            })
        })
    }

    /// Seconds remaining until sensor expiration (negative if expired).
    pub fn sensor_seconds_remaining(&self, now_epoch: i64) -> Option<i64> {
        self.effective_sensor_expires_at_epoch()
            .map(|exp| exp - now_epoch)
    }

    /// Returns `true` when the sensor has less than or equal to `threshold_hours` remaining
    /// (or has already expired).
    pub fn is_sensor_expiring_soon(&self, now_epoch: i64, threshold_hours: i32) -> bool {
        if threshold_hours <= 0 {
            return false;
        }
        self.sensor_seconds_remaining(now_epoch)
            .is_some_and(|rem| rem <= i64::from(threshold_hours) * 3600)
    }

    /// Formats a concise human-readable sensor warmup / expiry countdown string for the UI.
    pub fn format_sensor_status(&self, now_epoch: i64) -> String {
        if let Some(mins) = self.sensor_warmup_minutes_remaining(now_epoch) {
            return if self.sensor_serial.is_empty() {
                format!("⏳ Warmup: {mins}m remaining")
            } else {
                format!("⏳ Warmup: {mins}m remaining (SN: {})", self.sensor_serial)
            };
        }
        let Some(rem) = self.sensor_seconds_remaining(now_epoch) else {
            return if self.sensor_serial.is_empty() {
                "—".to_string()
            } else {
                format!("SN: {}", self.sensor_serial)
            };
        };
        let countdown = if rem <= 0 {
            "⚠ Sensor expired".to_string()
        } else if rem < 3600 {
            format!("⚠ Expires in {}m", (rem + 59) / 60)
        } else if rem < 86_400 {
            format!("⚠ {}h {}m remaining", rem / 3600, (rem % 3600) / 60)
        } else {
            format!("{}d {}h remaining", rem / 86_400, (rem % 86_400) / 3600)
        };
        if self.sensor_serial.is_empty() {
            countdown
        } else {
            format!("{countdown} ({})", self.sensor_serial)
        }
    }
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
        if value_mg_dl <= 0.0 || value_mg_dl.is_nan() {
            return false;
        }
        let low = self.effective_target_low(api_low);
        let high = self.effective_target_high(api_high);
        value_mg_dl < low || value_mg_dl > high
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

pub const MIN_UI_ZOOM_PERCENT: i32 = 75;
pub const MAX_UI_ZOOM_PERCENT: i32 = 175;

pub fn clamp_ui_zoom_percent(zoom: i32) -> i32 {
    if zoom <= 0 {
        100
    } else {
        zoom.clamp(MIN_UI_ZOOM_PERCENT, MAX_UI_ZOOM_PERCENT)
    }
}

/// Parses a `#RRGGBB` or `RRGGBB` hex color string into `(r, g, b)`.
pub fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() != 6 {
        return None;
    }
    let val = u32::from_str_radix(clean, 16).ok()?;
    Some(((val >> 16) as u8, ((val >> 8) & 0xff) as u8, (val & 0xff) as u8))
}

/// Computes the WCAG 2.1 relative luminance `L` in `[0.0, 1.0]` for an sRGB color.
pub fn wcag_relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    let channel = |c: u8| -> f64 {
        let s = f64::from(c) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// Computes the WCAG 2.1 contrast ratio `(L1 + 0.05) / (L2 + 0.05)` in `[1.0, 21.0]` between two
/// `#RRGGBB` hex colors.
pub fn wcag_contrast_ratio_hex(fg_hex: &str, bg_hex: &str) -> Option<f64> {
    let (r1, g1, b1) = parse_hex_rgb(fg_hex)?;
    let (r2, g2, b2) = parse_hex_rgb(bg_hex)?;
    let l1 = wcag_relative_luminance(r1, g1, b1);
    let l2 = wcag_relative_luminance(r2, g2, b2);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    Some((lighter + 0.05) / (darker + 0.05))
}

/// Builds a complete screen-reader announcement string for the current glucose snapshot.
pub fn accessible_glucose_announcement(
    snapshot: &GlucoseSnapshot,
    settings: &AppSettings,
    reading_stale: bool,
    age_display: &str,
) -> String {
    if snapshot.value_mg_dl <= 0.0 {
        return "No blood glucose reading available yet.".to_string();
    }
    let val = format_glucose_value(snapshot.value_mg_dl, &settings.result_unit);
    let trend = trend_arrow_description(snapshot.trend_arrow);
    let low = settings.effective_target_low(snapshot.target_low_mg_dl);
    let high = settings.effective_target_high(snapshot.target_high_mg_dl);
    let status = if reading_stale {
        "Stale reading"
    } else {
        GlucoseRangeStatus::classify(snapshot.value_mg_dl, low, high).label()
    };
    let target = format_target_range(low, high, &settings.result_unit);
    let age_part = if !age_display.trim().is_empty() {
        format!(", updated {}", age_display.trim())
    } else if !snapshot.timestamp.trim().is_empty() {
        format!(", measured at {}", snapshot.timestamp.trim())
    } else {
        String::new()
    };
    format!(
        "Blood glucose {} {}, trend {}, status {}{}. Target range {}.",
        val, settings.result_unit, trend, status, age_part, target
    )
}

pub fn load_settings_from_path(path: &Path) -> AppSettings {
    if let Ok(raw) = fs::read_to_string(path) {
        if let Ok(mut parsed) = serde_json::from_str::<AppSettings>(&raw) {
            parsed.poll_interval_seconds = clamp_poll_interval(parsed.poll_interval_seconds);
            parsed.ui_text_scale = clamp_ui_zoom_percent(parsed.ui_text_scale);
            return parsed;
        }
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

// ---------------------------------------------------------------------------
// Reading age / staleness and alert decision logic (pure, unit-testable).
// ---------------------------------------------------------------------------

/// Parses a LibreView timestamp such as `"10/19/2026 7:12:34 PM"` or an ISO 8601 / RFC 3339
/// timestamp from Home Assistant (e.g. `"2026-10-31T15:39:25.298895+00:00"`).
/// `as_utc = true` for `FactoryTimestamp` (UTC); `false` for `Timestamp` (local time).
pub fn parse_libreview_timestamp(raw: &str, as_utc: bool) -> Option<i64> {
    use chrono::{DateTime, NaiveDateTime, TimeZone};
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.timestamp());
    }
    let naive = ["%m/%d/%Y %I:%M:%S %p", "%m/%d/%Y %H:%M:%S", "%Y-%m-%dT%H:%M:%S"]
        .iter()
        .find_map(|fmt| NaiveDateTime::parse_from_str(trimmed, fmt).ok())?;
    if as_utc {
        Some(chrono::Utc.from_utc_datetime(&naive).timestamp())
    } else {
        chrono::Local
            .from_local_datetime(&naive)
            .earliest()
            .map(|dt| dt.timestamp())
    }
}

/// Whole minutes since the reading was measured (clamped at 0 for small clock skew).
pub fn reading_age_minutes(measured_at_epoch: Option<i64>, now_epoch: i64) -> Option<i64> {
    measured_at_epoch.map(|t| ((now_epoch - t).max(0)) / 60)
}

pub fn format_reading_age(age_minutes: Option<i64>) -> String {
    match age_minutes {
        None => String::new(),
        Some(0) => "just now".to_string(),
        Some(m) if m < 60 => format!("{m} min ago"),
        Some(m) => format!("{}h {:02}m ago", m / 60, m % 60),
    }
}

/// Kinds of alerts, ordered by severity (higher = more severe).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AlertKind {
    High,
    Stale,
    Low,
    UrgentLow,
}

impl AlertKind {
    pub fn title(&self) -> &'static str {
        match self {
            Self::UrgentLow => "URGENT LOW blood glucose",
            Self::Low => "Low blood glucose",
            Self::High => "High blood glucose",
            Self::Stale => "No recent glucose data",
        }
    }

    fn family(&self) -> u8 {
        match self {
            Self::Low | Self::UrgentLow => 0,
            Self::High => 1,
            Self::Stale => 2,
        }
    }

    /// Urgent low is the only alert that cannot be snoozed.
    pub fn bypasses_snooze(&self) -> bool {
        matches!(self, Self::UrgentLow)
    }

    /// Whether this alert warrants desktop-notification "critical" urgency.
    ///
    /// Stale (signal loss) and High are deliberately *not* critical: a dead sensor or a high
    /// reading should not raise an un-dismissable critical toast every repeat interval.
    pub fn is_critical(&self) -> bool {
        matches!(self, Self::UrgentLow | Self::Low)
    }
}

/// Interactive snooze action definitions `(action_id, button_label, minutes)` attached to
/// desktop alert notifications on Linux (org.freedesktop.Notifications) and Windows (WinRT Toast).
pub const ALERT_NOTIFICATION_SNOOZE_ACTIONS: &[(&str, &str, i32)] = &[
    ("snooze_30", "Snooze 30 min", 30),
    ("snooze_60", "Snooze 1 hr", 60),
    ("snooze_120", "Snooze 2 hr", 120),
];

/// Parses a desktop notification action identifier (e.g. `"snooze_30"`) into its snooze
/// duration in minutes (`Some(30)`), or returns `None` for non-snooze actions.
pub fn parse_notification_snooze_action(action_id: &str) -> Option<i32> {
    let clean = action_id.trim();
    ALERT_NOTIFICATION_SNOOZE_ACTIONS
        .iter()
        .find_map(|&(id, _, minutes)| (id == clean).then_some(minutes))
}

impl AppSettings {
    pub fn effective_urgent_low(&self) -> f64 {
        if self.urgent_low_mg_dl > 0.0 {
            self.urgent_low_mg_dl
        } else {
            DEFAULT_URGENT_LOW_MG_DL
        }
    }

    pub fn effective_stale_after_minutes(&self) -> i64 {
        self.stale_after_minutes.clamp(5, 120) as i64
    }

    pub fn is_reading_stale(&self, age_minutes: Option<i64>) -> bool {
        match age_minutes {
            Some(m) => m >= self.effective_stale_after_minutes(),
            // Fail closed. An unknown reading age (unparseable or missing timestamp) must be
            // treated as stale, never as a fresh reading: silently showing an old value as
            // live is the worst possible failure mode for a glucose monitor.
            None => true,
        }
    }

    pub fn is_snoozed(&self, now_epoch: i64) -> bool {
        self.snooze_until_epoch > now_epoch
    }

    /// Determines which alert (if any) the current state warrants.
    /// A stale reading is not judged by value; it raises a `Stale` alert instead.
    pub fn classify_alert(
        &self,
        value_mg_dl: f64,
        api_low: f64,
        api_high: f64,
        stale: bool,
    ) -> Option<AlertKind> {
        if stale {
            return self.alert_on_stale.then_some(AlertKind::Stale);
        }
        if value_mg_dl <= 0.0 || value_mg_dl.is_nan() {
            return None;
        }
        if value_mg_dl < self.effective_urgent_low() {
            Some(AlertKind::UrgentLow)
        } else if value_mg_dl < self.effective_target_low(api_low) {
            Some(AlertKind::Low)
        } else if value_mg_dl > self.effective_target_high(api_high) {
            Some(AlertKind::High)
        } else {
            None
        }
    }

    /// Decides whether to fire `kind` now, given the last fired alert `(kind, epoch)`.
    /// - Snooze silences everything except urgent low.
    /// - A new or more severe kind fires immediately (escalation).
    /// - The same kind repeats after the repeat interval (urgent low: at most 5 min).
    pub fn should_fire_alert(
        &self,
        kind: AlertKind,
        last_fired: Option<(AlertKind, i64)>,
        now_epoch: i64,
    ) -> bool {
        if self.is_snoozed(now_epoch) && !kind.bypasses_snooze() {
            return false;
        }
        let mut repeat_minutes = self.alert_repeat_minutes.clamp(1, 240);
        if kind == AlertKind::UrgentLow {
            repeat_minutes = repeat_minutes.min(URGENT_LOW_REPEAT_MINUTES);
        }
        match last_fired {
            None => true,
            Some((last_kind, at)) => {
                let escalated = kind > last_kind;
                // e.g. high -> low or low -> stale; urgent low easing to low is NOT a new
                // condition and follows the repeat timer.
                let new_condition = kind.family() != last_kind.family();
                escalated || new_condition || now_epoch - at >= repeat_minutes as i64 * 60
            }
        }
    }
}

#[cfg(test)]
mod alert_tests {
    use super::*;

    fn settings() -> AppSettings {
        AppSettings::default()
    }

    #[test]
    fn parses_libreview_timestamps() {
        let utc = parse_libreview_timestamp("10/19/2026 7:05:09 PM", true).unwrap();
        assert_eq!(utc, 1792436709);
        assert_eq!(parse_libreview_timestamp("1/2/2026 9:00:00 AM", true), Some(1767344400));
        assert_eq!(parse_libreview_timestamp("", true), None);
        assert_eq!(parse_libreview_timestamp("garbage", true), None);
    }

    #[test]
    fn age_and_formatting() {
        assert_eq!(reading_age_minutes(Some(1000), 1000 + 125), Some(2));
        assert_eq!(reading_age_minutes(Some(2000), 1000), Some(0), "clock skew clamps to 0");
        assert_eq!(reading_age_minutes(None, 1000), None);
        assert_eq!(format_reading_age(Some(0)), "just now");
        assert_eq!(format_reading_age(Some(7)), "7 min ago");
        assert_eq!(format_reading_age(Some(135)), "2h 15m ago");
    }

    #[test]
    fn staleness_threshold() {
        let s = settings();
        assert!(!s.is_reading_stale(Some(14)));
        assert!(s.is_reading_stale(Some(15)));
        // Fail closed: an unknown age is treated as stale, never as fresh.
        assert!(
            s.is_reading_stale(None),
            "unknown reading age must be reported as stale"
        );
    }

    #[test]
    fn alert_criticality_does_not_escalate_stale_or_high() {
        assert!(AlertKind::UrgentLow.is_critical());
        assert!(AlertKind::Low.is_critical());
        assert!(!AlertKind::High.is_critical());
        assert!(
            !AlertKind::Stale.is_critical(),
            "signal loss must not raise critical-urgency toasts forever"
        );
    }

    #[test]
    fn poll_jitter_stays_within_bounds_and_budget() {
        for base in [60u32, 90, 300, 3600] {
            for _ in 0..50 {
                let j = jittered_poll_interval(base);
                let span = (base / 10).max(1);
                assert!(j >= MIN_POLL_INTERVAL_SECONDS, "{base} -> {j}");
                assert!(j <= MAX_POLL_INTERVAL_SECONDS, "{base} -> {j}");
                assert!(
                    j.abs_diff(base) <= span,
                    "jitter too large: base {base} -> {j}"
                );
            }
        }
        // Floor/ceiling clamping still applies (jitter is applied before the clamp).
        let floor = jittered_poll_interval(0);
        assert!(
            (MIN_POLL_INTERVAL_SECONDS..=MIN_POLL_INTERVAL_SECONDS + 6).contains(&floor),
            "0 must clamp to the 60 s floor (+/-10% jitter), got {floor}"
        );
        let big = jittered_poll_interval(u32::MAX);
        assert!((MAX_POLL_INTERVAL_SECONDS - 360..=MAX_POLL_INTERVAL_SECONDS).contains(&big));
    }

    #[test]
    fn classification() {
        let s = settings();
        assert_eq!(s.classify_alert(50.0, 70.0, 180.0, false), Some(AlertKind::UrgentLow));
        assert_eq!(s.classify_alert(65.0, 70.0, 180.0, false), Some(AlertKind::Low));
        assert_eq!(s.classify_alert(120.0, 70.0, 180.0, false), None);
        assert_eq!(s.classify_alert(200.0, 70.0, 180.0, false), Some(AlertKind::High));
        assert_eq!(s.classify_alert(200.0, 70.0, 180.0, true), Some(AlertKind::Stale));
        let mut quiet = settings();
        quiet.alert_on_stale = false;
        assert_eq!(quiet.classify_alert(50.0, 70.0, 180.0, true), None);
    }

    #[test]
    fn repeat_interval_and_escalation() {
        let s = settings(); // 15 min repeat
        let t0 = 1_000_000;
        assert!(s.should_fire_alert(AlertKind::High, None, t0));
        assert!(!s.should_fire_alert(AlertKind::High, Some((AlertKind::High, t0)), t0 + 60));
        assert!(s.should_fire_alert(AlertKind::High, Some((AlertKind::High, t0)), t0 + 15 * 60));
        // Escalation fires immediately.
        assert!(s.should_fire_alert(AlertKind::UrgentLow, Some((AlertKind::Low, t0)), t0 + 60));
        // A different kind (e.g. high -> stale) also fires immediately.
        assert!(s.should_fire_alert(AlertKind::Stale, Some((AlertKind::High, t0)), t0 + 60));
        // Urgent low repeats every 5 min even with a longer configured interval.
        assert!(!s.should_fire_alert(AlertKind::UrgentLow, Some((AlertKind::UrgentLow, t0)), t0 + 240));
        assert!(s.should_fire_alert(AlertKind::UrgentLow, Some((AlertKind::UrgentLow, t0)), t0 + 300));
        // Easing from urgent low to low follows the repeat timer (no immediate re-alert).
        assert!(!s.should_fire_alert(AlertKind::Low, Some((AlertKind::UrgentLow, t0)), t0 + 60));
    }

    #[test]
    fn snooze_silences_all_but_urgent_low() {
        let t0 = 1_000_000;
        let mut s = settings();
        s.snooze_until_epoch = t0 + 1800;
        assert!(s.is_snoozed(t0));
        assert!(!s.should_fire_alert(AlertKind::High, None, t0));
        assert!(!s.should_fire_alert(AlertKind::Low, None, t0));
        assert!(!s.should_fire_alert(AlertKind::Stale, None, t0));
        assert!(s.should_fire_alert(AlertKind::UrgentLow, None, t0));
        assert!(s.should_fire_alert(AlertKind::High, None, t0 + 1800), "snooze expired");
    }

    #[test]
    fn notification_snooze_actions_parse_correctly() {
        assert_eq!(ALERT_NOTIFICATION_SNOOZE_ACTIONS.len(), 3);
        assert_eq!(parse_notification_snooze_action("snooze_30"), Some(30));
        assert_eq!(parse_notification_snooze_action("snooze_60"), Some(60));
        assert_eq!(parse_notification_snooze_action("snooze_120"), Some(120));
        assert_eq!(parse_notification_snooze_action(" snooze_30 "), Some(30));
        assert_eq!(parse_notification_snooze_action("default"), None);
        assert_eq!(parse_notification_snooze_action("__closed"), None);
        assert_eq!(parse_notification_snooze_action(""), None);
    }

    #[test]
    fn poll_floor_60s_and_snapshot_debug_redacts_pii() {
        let def = AppSettings::default();
        assert_eq!(def.poll_interval_seconds, 60);
        assert!(!def.remember_password);
        assert_eq!(def.stats_window, "14d");
        assert_eq!(def.window_x, -1);
        assert_eq!(def.window_y, -1);
        assert_eq!(def.window_width, 980);
        assert_eq!(def.window_height, 680);
        assert_eq!(clamp_poll_interval(15), 60);
        assert_eq!(clamp_poll_interval(60), 60);
        assert_eq!(clamp_poll_interval(99999), 3600);

        let snap = GlucoseSnapshot {
            value_mg_dl: 112.0,
            trend_arrow: 3,
            timestamp: "10/19/2026 7:05:09 PM".into(),
            target_low_mg_dl: 70.0,
            target_high_mg_dl: 180.0,
            patient_id: "pat-secret-uuid".into(),
            first_name: "Alice".into(),
            last_name: "Patient".into(),
            graph_points: vec![],
            measured_at_epoch: Some(1792436709),
            sensor_serial: "40TUAM8CTE0".into(),
            ..Default::default()
        };
        let dbg = format!("{snap:?}");
        assert!(!dbg.contains("pat-secret-uuid"));
        assert!(!dbg.contains("Alice"));
        assert!(!dbg.contains("Patient"));
        assert!(!dbg.contains("40TUAM8CTE0"));
        assert!(dbg.contains("[REDACTED]"));
    }

    #[test]
    fn wcag_contrast_ratios_meet_aa_thresholds() {
        // Light theme card background (#ffffff) vs slate-900 primary text (#0f172a) -> >= 15:1 (WCAG AAA)
        let light_card_text = wcag_contrast_ratio_hex("#0f172a", "#ffffff").unwrap();
        assert!(light_card_text >= 15.0, "got {light_card_text}");

        // Light theme alternating log row (#e2e8f0) vs primary text (#0f172a) -> >= 11:1 (WCAG AAA)
        let light_row_text = wcag_contrast_ratio_hex("#0f172a", "#e2e8f0").unwrap();
        assert!(light_row_text >= 11.0, "got {light_row_text}");

        // Light theme scrollbar thumb (#334155) vs track (#cbd5e1) -> >= 6.0:1 (WCAG AA UI >= 3.0)
        let sb_contrast = wcag_contrast_ratio_hex("#334155", "#cbd5e1").unwrap();
        assert!(sb_contrast >= 6.0, "got {sb_contrast}");

        // Darkened status hero/overlay fills (Qt.darker(..., 1.42)) vs #ffffff text -> >= 3.0:1 large text AA
        let dark_green_hero = wcag_contrast_ratio_hex("#ffffff", "#188a42").unwrap();
        assert!(dark_green_hero >= 4.2, "got {dark_green_hero}");
        let dark_red_hero = wcag_contrast_ratio_hex("#ffffff", "#a83030").unwrap();
        assert!(dark_red_hero >= 5.5, "got {dark_red_hero}");
    }

    #[test]
    fn ui_zoom_percent_clamping_and_accessible_announcement() {
        assert_eq!(clamp_ui_zoom_percent(0), 100);
        assert_eq!(clamp_ui_zoom_percent(50), 75);
        assert_eq!(clamp_ui_zoom_percent(100), 100);
        assert_eq!(clamp_ui_zoom_percent(125), 125);
        assert_eq!(clamp_ui_zoom_percent(250), 175);

        let s = AppSettings::default();
        let empty = GlucoseSnapshot::default();
        assert_eq!(
            accessible_glucose_announcement(&empty, &s, false, ""),
            "No blood glucose reading available yet."
        );

        let snap = GlucoseSnapshot {
            value_mg_dl: 112.0,
            trend_arrow: 3,
            timestamp: "10/31/2026 12:00:00 PM".into(),
            target_low_mg_dl: 70.0,
            target_high_mg_dl: 180.0,
            patient_id: "p1".into(),
            first_name: "Russell".into(),
            last_name: "Milliner".into(),
            graph_points: vec![],
            measured_at_epoch: Some(1_793_462_400),
            ..Default::default()
        };
        let ann = accessible_glucose_announcement(&snap, &s, false, "1 min ago");
        assert!(ann.contains("Blood glucose 112 mg/dL"));
        assert!(ann.contains("trend Stable"));
        assert!(ann.contains("status In Range"));
        assert!(ann.contains("updated 1 min ago"));
        assert!(ann.contains("Target range 70\u{2013}180 mg/dL"));
    }

    #[test]
    fn sensor_warmup_and_expiry_countdown_formatting() {
        let act = 1_700_000_000;
        let exp = act + 15 * 86_400;
        let snap = GlucoseSnapshot {
            value_mg_dl: 109.0,
            sensor_serial: "40TUAM8CTE0".into(),
            sensor_activated_at_epoch: Some(act),
            sensor_expires_at_epoch: Some(exp),
            sensor_lifespan_days: 15,
            sensor_plus_model: true,
            ..Default::default()
        };

        // 20 minutes after activation -> 40m warmup remaining
        assert!(snap.is_sensor_in_warmup(act + 20 * 60));
        assert_eq!(snap.sensor_warmup_minutes_remaining(act + 20 * 60), Some(40));
        let warm_str = snap.format_sensor_status(act + 20 * 60);
        assert!(warm_str.contains("Warmup: 40m remaining"), "got {warm_str}");
        assert!(warm_str.contains("40TUAM8CTE0"), "got {warm_str}");

        // 2 days after activation -> 13d 0h remaining
        assert!(!snap.is_sensor_in_warmup(act + 2 * 86_400));
        assert!(!snap.is_sensor_expiring_soon(act + 2 * 86_400, 24));
        let mid_str = snap.format_sensor_status(act + 2 * 86_400);
        assert!(mid_str.contains("13d 0h remaining"), "got {mid_str}");

        // 12 hours before expiration -> expiring soon
        assert!(snap.is_sensor_expiring_soon(exp - 12 * 3600, 24));
        let soon_str = snap.format_sensor_status(exp - 12 * 3600);
        assert!(soon_str.contains("12h 0m remaining"), "got {soon_str}");

        // After expiration -> expired
        assert!(snap.is_sensor_expiring_soon(exp + 60, 24));
        let exp_str = snap.format_sensor_status(exp + 60);
        assert!(exp_str.contains("Sensor expired"), "got {exp_str}");
    }
}
