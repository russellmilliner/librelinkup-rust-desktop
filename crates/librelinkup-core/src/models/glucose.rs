use std::fmt;

use serde::{Deserialize, Serialize};

use super::{
    DEFAULT_TARGET_HIGH_MG_DL, DEFAULT_TARGET_LOW_MG_DL, HIGH_CRITICAL_MG_DL, LOW_CRITICAL_MG_DL,
};

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
    if !value_mg_dl.is_finite() || value_mg_dl <= 0.0 {
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
    let naive = [
        "%m/%d/%Y %I:%M:%S %p",
        "%m/%d/%Y %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
    ]
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
    measured_at_epoch.map(|t| now_epoch.saturating_sub(t).max(0) / 60)
}

pub fn format_reading_age(age_minutes: Option<i64>) -> String {
    match age_minutes {
        None => String::new(),
        Some(0) => "just now".to_string(),
        Some(m) if m < 60 => format!("{m} min ago"),
        Some(m) => format!("{}h {:02}m ago", m / 60, m % 60),
    }
}
