//! Qt object declarations for the three QML controllers.
//!
//! The `#[qobject]` surface is declared here and implemented across
//! `telemetry.rs`, `settings.rs` and `medications.rs`. Shared application data lives in
//! `state.rs`; the display/analytics/persistence transitions live in `ops.rs`.
//!
//! Naming note: `TelemetryController` is also the **service/action host** - it is the Qt
//! object handed to the C++ tray and window managers, so it owns the async work (login,
//! refresh, update check/install) and delegates the settings-shaped actions into shared
//! state. See `docs/ARCHITECTURE.md`.

// The QML form bindings map 1:1 onto flat invokable argument lists (`upsert_medication`,
// `save_alert_settings`, `librelinkup_sync_tray`); collapsing them into parameter objects
// would only add boilerplate at every call site.
#![allow(clippy::too_many_arguments)]

use std::pin::Pin;

use cxx_qt_lib::QString;

/// Property storage for `TelemetryController` (one field per `#[qproperty]`).
pub struct TelemetryControllerRust {
    pub logged_in: bool,
    pub busy: bool,
    pub status_message: QString,
    pub error_message: QString,
    pub username: QString,
    pub patient_name: QString,
    pub patient_id: QString,
    pub glucose_display: QString,
    pub glucose_mg_dl: f64,
    pub result_unit: QString,
    pub trend_arrow: i32,
    pub trend_symbol: QString,
    pub trend_description: QString,
    pub range_status_label: QString,
    pub range_color_hex: QString,
    pub target_range_display: QString,
    pub target_low_mg_dl: f64,
    pub target_high_mg_dl: f64,
    pub last_updated: QString,
    pub graph_points_json: QString,
    pub reading_age_display: QString,
    pub reading_stale: bool,
    pub alerts_snoozed: bool,
    pub snooze_until_display: QString,
    pub app_version: QString,
    pub credentials_encrypted: bool,
    pub stats_window: QString,
    pub ea1c_headline: QString,
    pub ea1c_subline: QString,
    pub ea1c_display: QString,
    pub gmi_display: QString,
    pub tir_display: QString,
    pub accessible_summary: QString,
    pub sensor_status_display: QString,
    pub sensor_serial: QString,
    pub sensor_in_warmup: bool,
    pub sensor_expiring_soon: bool,
}

/// Property storage for `SettingsController` (one field per `#[qproperty]`).
pub struct SettingsControllerRust {
    pub theme_mode: QString,
    pub ui_zoom_percent: i32,
    pub high_contrast_mode: bool,
    pub country: QString,
    pub language: QString,
    pub window_mode: QString,
    pub overlay_visible: bool,
    pub overlay_only_mode: bool,
    pub overlay_on_all_desktops: bool,
    pub overlay_dark_theme: bool,
    pub overlay_opacity_percent: i32,
    pub overlay_screen_name: QString,
    pub overlay_x: i32,
    pub overlay_y: i32,
    pub overlay_width: i32,
    pub overlay_height: i32,
    pub window_screen_name: QString,
    pub window_x: i32,
    pub window_y: i32,
    pub window_width: i32,
    pub window_height: i32,
    pub show_glucose_in_tray_icon: bool,
    pub show_glucose_in_tray_tooltip: bool,
    pub autostart_enabled: bool,
    pub start_minimized: bool,
    pub poll_interval_seconds: i32,
    pub bring_to_front_enabled: bool,
    pub flash_window_enabled: bool,
    pub audio_alert_enabled: bool,
    pub use_custom_sound: bool,
    pub custom_sound_path: QString,
    pub override_threshold: bool,
    pub custom_target_low: f64,
    pub custom_target_high: f64,
    pub remember_password: bool,
    pub credentials_vault_path: QString,
    pub data_source: QString,
    pub ha_url: QString,
    pub ha_sensor_entity_id: QString,
    pub ha_discovered_patients_json: QString,
    pub ha_token_configured: bool,
    pub ha_sync_medications: bool,
    pub ha_sync_snooze: bool,
    pub sensor_expiry_alert_hours: i32,
    pub urgent_low_mg_dl: f64,
    pub alert_repeat_minutes: i32,
    pub stale_after_minutes: i32,
    pub alert_on_stale: bool,
    pub check_updates_on_startup: bool,
    pub update_available: bool,
    pub latest_version: QString,
    pub update_url: QString,
    pub update_download_url: QString,
    pub update_status_message: QString,
    pub checking_for_updates: bool,
    pub installing_update: bool,
    pub event_log_json: QString,
    pub event_log_count: i32,
}

/// Property storage for `MedicationsController` (one field per `#[qproperty]`).
pub struct MedicationsControllerRust {
    pub medications_json: QString,
    pub medication_doses_json: QString,
    pub graph_doses_json: QString,
    pub medications_summary: QString,
    pub medications_count: i32,
    pub medication_doses_count: i32,
}

#[cxx_qt::bridge]
pub mod qobject {
    /// Everything the Qt tray needs for one refresh. Bundled into a shared struct so the C++
    /// shim keeps a stable, readable signature as the tray grows.
    struct TrayPayload {
        theme_mode: QString,
        badge_text: QString,
        trend_symbol: QString,
        badge_color_hex: QString,
        status_header: QString,
        range_header: QString,
        tooltip_text: QString,
        /// Bit flags: 0x01 light-on-dark icon, 0x02 show value in icon, 0x04 show tooltip,
        /// 0x08 autostart, 0x10 overlay visible, 0x20 overlay on all desktops,
        /// 0x40 overlay-only mode.
        flags: u8,
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        include!("app_helper.h");
        type QString = cxx_qt_lib::QString;

        fn librelinkup_sync_tray(telemetry: Pin<&mut TelemetryController>, payload: TrayPayload);
        fn librelinkup_apply_app_theme(theme_mode: &QString);
        fn librelinkup_show_main_window();
        fn librelinkup_hide_main_window();
        fn librelinkup_restore_overlay_placement(
            screen_name: &QString,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        );
        fn librelinkup_restore_main_window_placement(
            screen_name: &QString,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        );
        fn librelinkup_apply_overlay_all_desktops(on_all_desktops: bool);
        fn librelinkup_begin_overlay_drag();
        fn librelinkup_update_overlay_drag();
        fn librelinkup_end_overlay_drag(settings_controller: Pin<&mut SettingsController>);
        fn librelinkup_popup_tray_menu();
        fn librelinkup_trigger_window_alert(bring_to_front: bool, flash_window: bool);
        fn librelinkup_play_alert_sound(custom_sound_path: &QString);
        fn librelinkup_show_notification(
            title: &QString,
            body: &QString,
            critical: bool,
            with_snooze_actions: bool,
        );
        fn librelinkup_show_medication_notification(
            medication_id: &QString,
            title: &QString,
            body: &QString,
        );
        fn librelinkup_request_background_autostart(enable: bool, start_minimized: bool) -> bool;
        fn librelinkup_quit_app();
        fn librelinkup_run_app(start_minimized: bool) -> i32;
        fn librelinkup_run_ui_overflow_tests() -> QString;
    }

    // ---------------------------------------------------------------------------------
    // TelemetryController: auth, CGM snapshot, display strings, analytics, tray sync.
    // ---------------------------------------------------------------------------------
    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, logged_in)]
        #[qproperty(bool, busy)]
        #[qproperty(QString, status_message)]
        #[qproperty(QString, error_message)]
        #[qproperty(QString, username)]
        #[qproperty(QString, patient_name)]
        #[qproperty(QString, patient_id)]
        #[qproperty(QString, glucose_display)]
        #[qproperty(f64, glucose_mg_dl)]
        #[qproperty(QString, result_unit)]
        #[qproperty(i32, trend_arrow)]
        #[qproperty(QString, trend_symbol)]
        #[qproperty(QString, trend_description)]
        #[qproperty(QString, range_status_label)]
        #[qproperty(QString, range_color_hex)]
        #[qproperty(QString, target_range_display)]
        #[qproperty(f64, target_low_mg_dl)]
        #[qproperty(f64, target_high_mg_dl)]
        #[qproperty(QString, last_updated)]
        #[qproperty(QString, graph_points_json)]
        #[qproperty(QString, reading_age_display)]
        #[qproperty(bool, reading_stale)]
        #[qproperty(bool, alerts_snoozed)]
        #[qproperty(QString, snooze_until_display)]
        #[qproperty(QString, app_version)]
        #[qproperty(bool, credentials_encrypted)]
        #[qproperty(QString, stats_window)]
        #[qproperty(QString, ea1c_headline)]
        #[qproperty(QString, ea1c_subline)]
        #[qproperty(QString, ea1c_display)]
        #[qproperty(QString, gmi_display)]
        #[qproperty(QString, tir_display)]
        #[qproperty(QString, accessible_summary)]
        #[qproperty(QString, sensor_status_display)]
        #[qproperty(QString, sensor_serial)]
        #[qproperty(bool, sensor_in_warmup)]
        #[qproperty(bool, sensor_expiring_soon)]
        type TelemetryController = super::TelemetryControllerRust;

        #[qinvokable]
        fn initialize(self: Pin<&mut TelemetryController>);

        /// Periodic tick from QML: applies pending async results, refreshes every derived
        /// string, syncs the tray and evaluates alerts. This is the only entry point the UI
        /// timer needs.
        #[qinvokable]
        fn tick(self: Pin<&mut TelemetryController>);

        #[qinvokable]
        fn login(
            self: Pin<&mut TelemetryController>,
            username: QString,
            password: QString,
            country: QString,
            language: QString,
        ) -> bool;

        #[qinvokable]
        fn logout(self: Pin<&mut TelemetryController>);

        #[qinvokable]
        fn save_home_assistant_config(
            self: Pin<&mut TelemetryController>,
            data_source: QString,
            ha_url: QString,
            ha_token: QString,
            ha_sensor_entity_id: QString,
        ) -> bool;

        #[qinvokable]
        fn auto_fill_home_assistant_from_env(self: Pin<&mut TelemetryController>) -> bool;

        #[qinvokable]
        fn select_ha_patient(self: Pin<&mut TelemetryController>, entity_id: QString);

        #[qinvokable]
        fn refresh_glucose_now(self: Pin<&mut TelemetryController>);

        /// Next poll delay in milliseconds, jittered by +/-10% so restarts and multiple
        /// instances never hit the LibreView API in lock-step.
        #[qinvokable]
        fn next_poll_interval_ms(self: Pin<&mut TelemetryController>) -> i32;

        #[qinvokable]
        fn evaluate_staleness(self: Pin<&mut TelemetryController>);

        #[qinvokable]
        fn load_demo_sample_data(self: Pin<&mut TelemetryController>);

        #[qinvokable]
        fn update_stats_window(self: Pin<&mut TelemetryController>, window_code: QString);

        #[qinvokable]
        fn handle_notification_action(self: Pin<&mut TelemetryController>, action_key: QString) -> bool;

        #[qinvokable]
        fn snooze_alerts(self: Pin<&mut TelemetryController>, minutes: i32);

        #[qinvokable]
        fn check_for_updates(self: Pin<&mut TelemetryController>);

        #[qinvokable]
        fn check_for_updates_silent(self: Pin<&mut TelemetryController>);

        #[qinvokable]
        fn install_update(self: Pin<&mut TelemetryController>);

        // ---- action delegates invoked by the C++ tray / window manager ----
        #[qinvokable]
        fn update_theme_mode(self: Pin<&mut TelemetryController>, mode: QString);

        #[qinvokable]
        fn update_show_glucose_in_tray_icon(self: Pin<&mut TelemetryController>, enabled: bool);

        #[qinvokable]
        fn update_show_glucose_in_tray_tooltip(self: Pin<&mut TelemetryController>, enabled: bool);

        #[qinvokable]
        fn update_overlay_visible(self: Pin<&mut TelemetryController>, visible: bool);

        #[qinvokable]
        fn update_overlay_on_all_desktops(self: Pin<&mut TelemetryController>, enabled: bool);

        #[qinvokable]
        fn update_overlay_only_mode(self: Pin<&mut TelemetryController>, only_overlay: bool);

        #[qinvokable]
        fn update_autostart_enabled(self: Pin<&mut TelemetryController>, enabled: bool);

        #[qinvokable]
        fn refresh_glucose_from_tray(self: Pin<&mut TelemetryController>);

        #[qinvokable]
        fn show_main_window(self: Pin<&mut TelemetryController>);

        #[qinvokable]
        fn hide_main_window(self: Pin<&mut TelemetryController>);

        #[qinvokable]
        fn quit_application(self: Pin<&mut TelemetryController>);

        #[qinvokable]
        fn popup_tray_menu_at_cursor(self: Pin<&mut TelemetryController>);
    }

    impl cxx_qt::Threading for TelemetryController {}

    // ---------------------------------------------------------------------------------
    // SettingsController: preferences, window/overlay, alerts config, updater status, logs.
    // ---------------------------------------------------------------------------------
    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, theme_mode)]
        #[qproperty(i32, ui_zoom_percent)]
        #[qproperty(bool, high_contrast_mode)]
        #[qproperty(QString, country)]
        #[qproperty(QString, language)]
        #[qproperty(QString, window_mode)]
        #[qproperty(bool, overlay_visible)]
        #[qproperty(bool, overlay_only_mode)]
        #[qproperty(bool, overlay_on_all_desktops)]
        #[qproperty(bool, overlay_dark_theme)]
        #[qproperty(i32, overlay_opacity_percent)]
        #[qproperty(QString, overlay_screen_name)]
        #[qproperty(i32, overlay_x)]
        #[qproperty(i32, overlay_y)]
        #[qproperty(i32, overlay_width)]
        #[qproperty(i32, overlay_height)]
        #[qproperty(QString, window_screen_name)]
        #[qproperty(i32, window_x)]
        #[qproperty(i32, window_y)]
        #[qproperty(i32, window_width)]
        #[qproperty(i32, window_height)]
        #[qproperty(bool, show_glucose_in_tray_icon)]
        #[qproperty(bool, show_glucose_in_tray_tooltip)]
        #[qproperty(bool, autostart_enabled)]
        #[qproperty(bool, start_minimized)]
        #[qproperty(i32, poll_interval_seconds)]
        #[qproperty(bool, bring_to_front_enabled)]
        #[qproperty(bool, flash_window_enabled)]
        #[qproperty(bool, audio_alert_enabled)]
        #[qproperty(bool, use_custom_sound)]
        #[qproperty(QString, custom_sound_path)]
        #[qproperty(bool, override_threshold)]
        #[qproperty(f64, custom_target_low)]
        #[qproperty(f64, custom_target_high)]
        #[qproperty(bool, remember_password)]
        #[qproperty(QString, credentials_vault_path)]
        #[qproperty(QString, data_source)]
        #[qproperty(QString, ha_url)]
        #[qproperty(QString, ha_sensor_entity_id)]
        #[qproperty(QString, ha_discovered_patients_json)]
        #[qproperty(bool, ha_token_configured)]
        #[qproperty(bool, ha_sync_medications)]
        #[qproperty(bool, ha_sync_snooze)]
        #[qproperty(i32, sensor_expiry_alert_hours)]
        #[qproperty(f64, urgent_low_mg_dl)]
        #[qproperty(i32, alert_repeat_minutes)]
        #[qproperty(i32, stale_after_minutes)]
        #[qproperty(bool, alert_on_stale)]
        #[qproperty(bool, check_updates_on_startup)]
        #[qproperty(bool, update_available)]
        #[qproperty(QString, latest_version)]
        #[qproperty(QString, update_url)]
        #[qproperty(QString, update_download_url)]
        #[qproperty(QString, update_status_message)]
        #[qproperty(bool, checking_for_updates)]
        #[qproperty(bool, installing_update)]
        #[qproperty(QString, event_log_json)]
        #[qproperty(i32, event_log_count)]
        type SettingsController = super::SettingsControllerRust;

        #[qinvokable]
        fn initialize(self: Pin<&mut SettingsController>);

        /// Mirrors shared state into QML properties (called by the UI timer).
        #[qinvokable]
        fn sync(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn update_theme_mode(self: Pin<&mut SettingsController>, mode: QString);

        #[qinvokable]
        fn update_ui_zoom_percent(self: Pin<&mut SettingsController>, zoom_percent: i32);

        #[qinvokable]
        fn zoom_in(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn zoom_out(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn reset_zoom(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn update_high_contrast_mode(self: Pin<&mut SettingsController>, enabled: bool);

        #[qinvokable]
        fn update_data_source(self: Pin<&mut SettingsController>, data_source: QString);

        #[qinvokable]
        fn update_ha_sync_medications(self: Pin<&mut SettingsController>, enabled: bool);

        #[qinvokable]
        fn update_ha_sync_snooze(self: Pin<&mut SettingsController>, enabled: bool);

        #[qinvokable]
        fn update_sensor_expiry_alert_hours(self: Pin<&mut SettingsController>, hours: i32);

        #[qinvokable]
        fn update_result_unit(self: Pin<&mut SettingsController>, unit: QString);

        #[qinvokable]
        fn update_country(self: Pin<&mut SettingsController>, country: QString);

        #[qinvokable]
        fn update_remember_password(self: Pin<&mut SettingsController>, enabled: bool);

        #[qinvokable]
        fn update_window_mode(self: Pin<&mut SettingsController>, mode: QString);

        #[qinvokable]
        fn update_overlay_visible(self: Pin<&mut SettingsController>, visible: bool);

        #[qinvokable]
        fn update_overlay_only_mode(self: Pin<&mut SettingsController>, only_overlay: bool);

        #[qinvokable]
        fn update_overlay_on_all_desktops(self: Pin<&mut SettingsController>, enabled: bool);

        #[qinvokable]
        fn update_overlay_dark_theme(self: Pin<&mut SettingsController>, enabled: bool);

        #[qinvokable]
        fn update_overlay_opacity_percent(self: Pin<&mut SettingsController>, opacity_percent: i32);

        #[qinvokable]
        fn save_overlay_placement(
            self: Pin<&mut SettingsController>,
            screen_name: QString,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        );

        #[qinvokable]
        fn save_main_window_placement(
            self: Pin<&mut SettingsController>,
            screen_name: QString,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        );

        #[qinvokable]
        fn begin_overlay_drag(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn update_overlay_drag(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn end_overlay_drag(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn popup_tray_menu_at_cursor(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn update_show_glucose_in_tray_icon(self: Pin<&mut SettingsController>, enabled: bool);

        #[qinvokable]
        fn update_show_glucose_in_tray_tooltip(self: Pin<&mut SettingsController>, enabled: bool);

        #[qinvokable]
        fn update_autostart_enabled(self: Pin<&mut SettingsController>, enabled: bool);

        #[qinvokable]
        fn update_start_minimized(self: Pin<&mut SettingsController>, enabled: bool);

        #[qinvokable]
        fn update_poll_interval_seconds(self: Pin<&mut SettingsController>, seconds: i32);

        #[qinvokable]
        fn save_alert_settings(
            self: Pin<&mut SettingsController>,
            bring_to_front: bool,
            flash_window: bool,
            audio_alert: bool,
            use_custom_sound: bool,
            custom_sound_path: QString,
            override_threshold: bool,
            custom_low: f64,
            custom_high: f64,
        );

        #[qinvokable]
        fn test_alert_now(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn save_advanced_alert_settings(
            self: Pin<&mut SettingsController>,
            urgent_low: f64,
            repeat_minutes: i32,
            stale_after_minutes: i32,
            alert_on_stale: bool,
        );

        #[qinvokable]
        fn update_check_updates_on_startup(self: Pin<&mut SettingsController>, enabled: bool);

        #[qinvokable]
        fn refresh_event_logs(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn clear_event_logs(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn export_event_logs(self: Pin<&mut SettingsController>, path: QString) -> bool;

        #[qinvokable]
        fn show_main_window(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn hide_main_window(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn quit_application(self: Pin<&mut SettingsController>);
    }

    impl cxx_qt::Threading for SettingsController {}

    // ---------------------------------------------------------------------------------
    // MedicationsController: schedule + dose log.
    // ---------------------------------------------------------------------------------
    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, medications_json)]
        #[qproperty(QString, medication_doses_json)]
        #[qproperty(QString, graph_doses_json)]
        #[qproperty(QString, medications_summary)]
        #[qproperty(i32, medications_count)]
        #[qproperty(i32, medication_doses_count)]
        type MedicationsController = super::MedicationsControllerRust;

        #[qinvokable]
        fn initialize(self: Pin<&mut MedicationsController>);

        #[qinvokable]
        fn sync(self: Pin<&mut MedicationsController>);

        #[qinvokable]
        fn upsert_medication(
            self: Pin<&mut MedicationsController>,
            id: QString,
            name: QString,
            dosage: QString,
            schedule: QString,
            scheduled_time: QString,
            weekday: i32,
            reminder_enabled: bool,
            notes: QString,
        ) -> bool;

        #[qinvokable]
        fn remove_medication(self: Pin<&mut MedicationsController>, medication_id: QString) -> bool;

        #[qinvokable]
        fn log_medication_dose(
            self: Pin<&mut MedicationsController>,
            medication_id: QString,
            custom_dosage: QString,
            notes: QString,
        ) -> bool;

        #[qinvokable]
        fn skip_medication_dose(
            self: Pin<&mut MedicationsController>,
            medication_id: QString,
            notes: QString,
        ) -> bool;

        #[qinvokable]
        fn remove_medication_dose(self: Pin<&mut MedicationsController>, dose_id: QString) -> bool;
    }

    impl cxx_qt::Threading for MedicationsController {}
}
