use serde::{Deserialize, Serialize};

use super::{AppSettings, DEFAULT_URGENT_LOW_MG_DL, URGENT_LOW_REPEAT_MINUTES};

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
        if !value_mg_dl.is_finite() || value_mg_dl <= 0.0 {
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

