use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::paths::settings_file_path;

pub const LOW_CRITICAL_MG_DL: f64 = 70.0;
pub const HIGH_CRITICAL_MG_DL: f64 = 240.0;
pub const DEFAULT_TARGET_LOW_MG_DL: f64 = 70.0;
pub const DEFAULT_TARGET_HIGH_MG_DL: f64 = 180.0;
pub const DEFAULT_URGENT_LOW_MG_DL: f64 = 55.0;
pub const DEFAULT_ALERT_REPEAT_MINUTES: u32 = 15;
pub const DEFAULT_STALE_AFTER_MINUTES: u32 = 15;
/// Urgent-low alerts repeat at least this often regardless of the configured repeat interval.
pub const URGENT_LOW_REPEAT_MINUTES: u32 = 5;
/// Gray used for stale readings in the UI, tray badge and overlay.
pub const STALE_COLOR_HEX: &str = "#6b7280";

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    /// Check GitHub Releases for a newer version automatically at startup.
    pub check_updates_on_startup: bool,
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
            show_glucose_in_tray_icon: true,
            show_glucose_in_tray_tooltip: true,
            tray_icon_theme: "auto".to_string(),
            autostart_enabled: true,
            start_minimized: true,
            poll_interval_seconds: 60,
            bring_to_front_enabled: false,
            flash_window_enabled: true,
            audio_alert_enabled: true,
            use_custom_sound: false,
            custom_sound_path: String::new(),
            override_threshold: false,
            custom_target_low: DEFAULT_TARGET_LOW_MG_DL,
            custom_target_high: DEFAULT_TARGET_HIGH_MG_DL,
            ui_text_scale: 100,
            urgent_low_mg_dl: DEFAULT_URGENT_LOW_MG_DL,
            alert_repeat_minutes: DEFAULT_ALERT_REPEAT_MINUTES,
            stale_after_minutes: DEFAULT_STALE_AFTER_MINUTES,
            alert_on_stale: true,
            snooze_until_epoch: 0,
            check_updates_on_startup: true,
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

pub fn load_settings_from_path(path: &Path) -> AppSettings {
    if let Ok(raw) = fs::read_to_string(path) {
        if let Ok(parsed) = serde_json::from_str::<AppSettings>(&raw) {
            return parsed;
        }
    }
    AppSettings::default()
}

pub fn save_settings_to_path(settings: &AppSettings, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    fs::write(path, json).map_err(|e| format!("Failed to write settings file: {e}"))
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

/// Parses a LibreView timestamp such as `"10/19/2026 7:12:34 PM"`.
/// `as_utc = true` for `FactoryTimestamp` (UTC); `false` for `Timestamp` (local time).
pub fn parse_libreview_timestamp(raw: &str, as_utc: bool) -> Option<i64> {
    use chrono::{NaiveDateTime, TimeZone};
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
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
        age_minutes.is_some_and(|m| m >= self.effective_stale_after_minutes())
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
        assert!(!s.is_reading_stale(None), "unknown age is not treated as stale");
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
}
