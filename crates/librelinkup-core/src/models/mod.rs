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

mod a11y;
mod alerts;
mod glucose;
mod settings;

pub use a11y::*;
pub use alerts::*;
pub use glucose::*;
pub use settings::*;

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
        assert_eq!(
            parse_libreview_timestamp("1/2/2026 9:00:00 AM", true),
            Some(1767344400)
        );
        assert_eq!(parse_libreview_timestamp("", true), None);
        assert_eq!(parse_libreview_timestamp("garbage", true), None);
    }

    #[test]
    fn age_and_formatting() {
        assert_eq!(reading_age_minutes(Some(1000), 1000 + 125), Some(2));
        assert_eq!(
            reading_age_minutes(Some(2000), 1000),
            Some(0),
            "clock skew clamps to 0"
        );
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
        assert_eq!(
            s.classify_alert(50.0, 70.0, 180.0, false),
            Some(AlertKind::UrgentLow)
        );
        assert_eq!(
            s.classify_alert(65.0, 70.0, 180.0, false),
            Some(AlertKind::Low)
        );
        assert_eq!(s.classify_alert(120.0, 70.0, 180.0, false), None);
        assert_eq!(
            s.classify_alert(200.0, 70.0, 180.0, false),
            Some(AlertKind::High)
        );
        assert_eq!(
            s.classify_alert(200.0, 70.0, 180.0, true),
            Some(AlertKind::Stale)
        );
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
        assert!(!s.should_fire_alert(
            AlertKind::UrgentLow,
            Some((AlertKind::UrgentLow, t0)),
            t0 + 240
        ));
        assert!(s.should_fire_alert(
            AlertKind::UrgentLow,
            Some((AlertKind::UrgentLow, t0)),
            t0 + 300
        ));
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
        assert!(
            s.should_fire_alert(AlertKind::High, None, t0 + 1800),
            "snooze expired"
        );
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
        assert_eq!(
            snap.sensor_warmup_minutes_remaining(act + 20 * 60),
            Some(40)
        );
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

    #[test]
    fn tray_only_relevant_out_of_range_classification() {
        let mut s = AppSettings::default();
        assert!(!s.tray_only_relevant_out_of_range);

        let in_range = GlucoseSnapshot {
            value_mg_dl: 112.0,
            target_low_mg_dl: 70.0,
            target_high_mg_dl: 180.0,
            ..Default::default()
        };
        let high = GlucoseSnapshot {
            value_mg_dl: 215.0,
            target_low_mg_dl: 70.0,
            target_high_mg_dl: 180.0,
            ..Default::default()
        };
        let low = GlucoseSnapshot {
            value_mg_dl: 64.0,
            target_low_mg_dl: 70.0,
            target_high_mg_dl: 180.0,
            ..Default::default()
        };
        let urgent_low = GlucoseSnapshot {
            value_mg_dl: 49.0,
            target_low_mg_dl: 70.0,
            target_high_mg_dl: 180.0,
            ..Default::default()
        };
        let waiting = GlucoseSnapshot::default();

        // Default (false): always relevant (Active)
        assert!(s.is_tray_relevant(&in_range, false));
        assert!(s.is_tray_relevant(&waiting, false));

        // Enabled (true): Passive when waiting or fresh In Range; Active when High, Low, UrgentLow, or Stale
        s.tray_only_relevant_out_of_range = true;
        assert!(!s.is_tray_relevant(&waiting, false));
        assert!(!s.is_tray_relevant(&in_range, false));
        assert!(
            s.is_tray_relevant(&in_range, true),
            "stale reading is relevant"
        );
        assert!(s.is_tray_relevant(&high, false));
        assert!(s.is_tray_relevant(&low, false));
        assert!(s.is_tray_relevant(&urgent_low, false));
        assert!(!s.is_tray_urgent_attention(&low, false));
        assert!(s.is_tray_urgent_attention(&urgent_low, false));
        assert!(!s.is_tray_urgent_attention(&urgent_low, true));
    }
}
