use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::paths::settings_file_path;

pub const LOW_CRITICAL_MG_DL: f64 = 70.0;
pub const HIGH_CRITICAL_MG_DL: f64 = 240.0;
pub const DEFAULT_TARGET_LOW_MG_DL: f64 = 70.0;
pub const DEFAULT_TARGET_HIGH_MG_DL: f64 = 180.0;

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
