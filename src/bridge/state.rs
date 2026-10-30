//! Shared application state for the three QML controllers.
//!
//! ## Why this exists
//!
//! The Qt front-end used to be one `AppController` object with ~70 properties and ~35
//! invokables. Every feature widened it, and any background result had to be delivered
//! through that single object. The state now lives here, behind a Qt-GUI-thread-only
//! `RefCell`, and three small controllers expose disjoint slices of it:
//!
//! | Controller              | Owns                                                        |
//! |-------------------------|-------------------------------------------------------------|
//! | `TelemetryController`   | auth, CGM snapshot, display strings, analytics, tray sync    |
//! | `SettingsController`    | preferences, window/overlay, upater, diagnostics (log)       |
//! | `MedicationsController` | medication schedule, dose log                                |
//!
//! ## Threading contract (important for future agents)
//!
//! * [`with_state`] **must only be called from the Qt GUI thread.** All cxx-qt invokables and
//!   property setters run there, and every background task funnels results back with
//!   `qt_thread().queue(...)`, which also runs there.
//! * Never hold a `with_state` borrow across a call that can re-enter (e.g. do not call a
//!   `sync()` method inside a `with_state` closure). Clone the values out first.
//! * Cross-controller refresh is pull-based: `Main.qml` runs `sync()` on the controllers from
//!   a timer, and each controller only pushes a property when its value actually changed.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::analytics::{load_default_glucose_history, GlucoseHistoryStore};
use crate::crypto::StoredCredentials;
use crate::medications::{load_default_medications, MedicationStore};
use crate::models::{GlucoseSnapshot, AlertKind};

pub struct AppState {
    // ---- telemetry ----
    pub logged_in: bool,
    pub busy: bool,
    pub status_message: String,
    pub error_message: String,
    pub username: String,
    pub patient_name: String,
    pub patient_id: String,
    pub glucose_display: String,
    pub glucose_mg_dl: f64,
    pub result_unit: String,
    pub trend_arrow: i32,
    pub trend_symbol: String,
    pub trend_description: String,
    pub range_status_label: String,
    pub range_color_hex: String,
    pub target_range_display: String,
    pub target_low_mg_dl: f64,
    pub target_high_mg_dl: f64,
    pub last_updated: String,
    pub graph_points_json: String,
    pub reading_age_display: String,
    pub reading_stale: bool,
    pub alerts_snoozed: bool,
    pub snooze_until_display: String,
    /// Epoch of the most recent successful CGM fetch (drives the fail-closed staleness clock).
    pub last_successful_fetch_epoch: Option<i64>,
    /// Epoch of the last encrypted-history write; throttles the vault rewrite.
    pub last_history_persist_epoch: i64,
    pub stats_window: String,
    pub ea1c_headline: String,
    pub ea1c_subline: String,
    pub ea1c_display: String,
    pub gmi_display: String,
    pub tir_display: String,
    pub snooze_until_epoch: i64,
    pub last_alert: Option<(AlertKind, i64)>,
    pub snapshot: GlucoseSnapshot,
    pub creds: Option<StoredCredentials>,
    pub glucose_history: GlucoseHistoryStore,
    pub active_refresh: Option<u64>,
    pub refresh_seq: u64,
    pub login_in_flight: bool,
    pub session_generation: u64,

    // ---- settings ----
    pub country: String,
    pub language: String,
    pub theme_mode: String,
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
    pub autostart_enabled: bool,
    pub start_minimized: bool,
    pub poll_interval_seconds: i32,
    pub bring_to_front_enabled: bool,
    pub flash_window_enabled: bool,
    pub audio_alert_enabled: bool,
    pub use_custom_sound: bool,
    pub custom_sound_path: String,
    pub override_threshold: bool,
    pub custom_target_low: f64,
    pub custom_target_high: f64,
    pub credentials_encrypted: bool,
    pub remember_password: bool,
    pub credentials_vault_path: String,
    pub urgent_low_mg_dl: f64,
    pub alert_repeat_minutes: i32,
    pub stale_after_minutes: i32,
    pub alert_on_stale: bool,
    pub check_updates_on_startup: bool,
    pub update_available: bool,
    pub latest_version: String,
    pub update_url: String,
    pub update_download_url: String,
    pub update_status_message: String,
    pub checking_for_updates: bool,
    pub installing_update: bool,
    pub last_notified_update_version: String,
    pub last_update_check_epoch: i64,
    pub event_log_json: String,
    pub event_log_count: i32,

    // ---- medications ----
    pub medication_store: MedicationStore,
    pub medication_last_notified: HashMap<String, i64>,
    pub medications_json: String,
    pub medication_doses_json: String,
    pub graph_doses_json: String,
    pub medications_summary: String,
    pub medications_count: i32,
    pub medication_doses_count: i32,

    // ---- constants ----
    pub app_version: String,
}

impl Default for AppState {
    fn default() -> Self {
        let settings = crate::models::load_default_settings();
        let unit = settings.result_unit.clone();
        let now = chrono::Utc::now().timestamp();
        Self {
            logged_in: false,
            busy: false,
            status_message: "Ready".to_string(),
            error_message: String::new(),
            username: String::new(),
            patient_name: String::new(),
            patient_id: String::new(),
            glucose_display: "--".to_string(),
            glucose_mg_dl: 0.0,
            result_unit: unit.clone(),
            trend_arrow: 3,
            trend_symbol: "→".to_string(),
            trend_description: "Stable".to_string(),
            range_status_label: "Waiting".to_string(),
            range_color_hex: "#22c55e".to_string(),
            target_range_display: crate::models::format_target_range(
                crate::models::DEFAULT_TARGET_LOW_MG_DL,
                crate::models::DEFAULT_TARGET_HIGH_MG_DL,
                &unit,
            ),
            target_low_mg_dl: crate::models::DEFAULT_TARGET_LOW_MG_DL,
            target_high_mg_dl: crate::models::DEFAULT_TARGET_HIGH_MG_DL,
            last_updated: "Never".to_string(),
            graph_points_json: "[]".to_string(),
            reading_age_display: String::new(),
            reading_stale: false,
            alerts_snoozed: settings.is_snoozed(now),
            snooze_until_display: crate::session::format_snooze_until(
                settings.snooze_until_epoch,
                now,
            ),
            last_successful_fetch_epoch: None,
            last_history_persist_epoch: 0,
            stats_window: crate::analytics::StatsWindow::from_code(&settings.stats_window)
                .code()
                .to_string(),
            ea1c_headline: "eA1C --% • GMI --% • TIR --%".to_string(),
            ea1c_subline: "Accumulating CGM readings for A1C / GMI prediction...".to_string(),
            ea1c_display: "--%".to_string(),
            gmi_display: "--%".to_string(),
            tir_display: "--%".to_string(),
            snooze_until_epoch: settings.snooze_until_epoch,
            last_alert: None,
            snapshot: GlucoseSnapshot::default(),
            creds: None,
            glucose_history: load_default_glucose_history(),
            active_refresh: None,
            refresh_seq: 0,
            login_in_flight: false,
            session_generation: 0,

            country: crate::api::normalize_country_code(&settings.country),
            language: settings.language.clone(),
            theme_mode: settings.theme_mode.clone(),
            window_mode: settings.window_mode.clone(),
            overlay_visible: settings.overlay_visible
                || settings.window_mode == "overlay"
                || settings.window_mode == "overlayTransparent",
            overlay_only_mode: settings.overlay_only_mode,
            overlay_on_all_desktops: settings.overlay_on_all_desktops,
            overlay_dark_theme: settings.overlay_dark_theme,
            overlay_opacity_percent: settings.overlay_opacity_percent.clamp(20, 100),
            overlay_screen_name: settings.overlay_screen_name.clone(),
            overlay_x: settings.overlay_x,
            overlay_y: settings.overlay_y,
            overlay_width: settings.overlay_width.clamp(230, 600),
            overlay_height: settings.overlay_height.clamp(80, 260),
            show_glucose_in_tray_icon: settings.show_glucose_in_tray_icon,
            show_glucose_in_tray_tooltip: settings.show_glucose_in_tray_tooltip,
            autostart_enabled: settings.autostart_enabled,
            start_minimized: settings.start_minimized,
            poll_interval_seconds: crate::models::clamp_poll_interval(
                settings.poll_interval_seconds,
            ) as i32,
            bring_to_front_enabled: settings.bring_to_front_enabled,
            flash_window_enabled: settings.flash_window_enabled,
            audio_alert_enabled: settings.audio_alert_enabled,
            use_custom_sound: settings.use_custom_sound,
            custom_sound_path: settings.custom_sound_path.clone(),
            override_threshold: settings.override_threshold,
            custom_target_low: settings.custom_target_low,
            custom_target_high: settings.custom_target_high,
            credentials_encrypted: crate::paths::credentials_file_path().exists(),
            remember_password: settings.remember_password,
            credentials_vault_path: crate::paths::credentials_file_path()
                .to_string_lossy()
                .into_owned(),
            urgent_low_mg_dl: settings.effective_urgent_low(),
            alert_repeat_minutes: settings.alert_repeat_minutes.clamp(1, 240) as i32,
            stale_after_minutes: settings.effective_stale_after_minutes() as i32,
            alert_on_stale: settings.alert_on_stale,
            check_updates_on_startup: settings.check_updates_on_startup,
            update_available: false,
            latest_version: format!("v{}", env!("CARGO_PKG_VERSION")),
            update_url: crate::updater::GITHUB_RELEASES_PAGE_URL.to_string(),
            update_download_url: crate::updater::GITHUB_RELEASES_PAGE_URL.to_string(),
            update_status_message: format!("Current version: v{}", env!("CARGO_PKG_VERSION")),
            checking_for_updates: false,
            installing_update: false,
            last_notified_update_version: String::new(),
            last_update_check_epoch: 0,
            event_log_json: "[]".to_string(),
            event_log_count: 0,

            medication_store: load_default_medications(),
            medication_last_notified: HashMap::new(),
            medications_json: "[]".to_string(),
            medication_doses_json: "[]".to_string(),
            graph_doses_json: "[]".to_string(),
            medications_summary: "No medications configured".to_string(),
            medications_count: 0,
            medication_doses_count: 0,

            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

thread_local! {
    static STATE: RefCell<AppState> = RefCell::new(AppState::default());
}

/// Runs `f` against the shared state. **Qt GUI thread only** - see the module docs.
pub fn with_state<R>(f: impl FnOnce(&mut AppState) -> R) -> R {
    STATE.with(|cell| f(&mut cell.borrow_mut()))
}

/// Replaces the shared state (used by tests that need a clean slate).
pub fn reset_state() {
    STATE.with(|cell| *cell.borrow_mut() = AppState::default());
}

/// Cheap monotonic-ish epoch seconds (UTC).
pub fn now_epoch() -> i64 {
    chrono::Utc::now().timestamp()
}
