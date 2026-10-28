#![allow(clippy::too_many_arguments)]

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        include!("app_helper.h");
        type QString = cxx_qt_lib::QString;

        fn librelinkup_sync_tray(
            controller: Pin<&mut AppController>,
            app_theme_mode: &QString,
            badge_text: &QString,
            trend_symbol: &QString,
            badge_color_hex: &QString,
            status_header: &QString,
            range_header: &QString,
            tooltip_text: &QString,
            flags: u8,
        );
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
        fn librelinkup_apply_overlay_all_desktops(on_all_desktops: bool);
        fn librelinkup_begin_overlay_drag();
        fn librelinkup_update_overlay_drag();
        fn librelinkup_end_overlay_drag(controller: Pin<&mut AppController>);
        fn librelinkup_popup_tray_menu();
        fn librelinkup_trigger_window_alert(bring_to_front: bool, flash_window: bool);
        fn librelinkup_play_alert_sound(custom_sound_path: &QString);
        fn librelinkup_show_notification(
            title: &QString,
            body: &QString,
            critical: bool,
            with_snooze_actions: bool,
        );
        fn librelinkup_request_background_autostart(enable: bool, start_minimized: bool) -> bool;
        fn librelinkup_quit_app();
        fn librelinkup_run_app(start_minimized: bool) -> i32;
        fn librelinkup_run_ui_overflow_tests() -> QString;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, logged_in)]
        #[qproperty(bool, busy)]
        #[qproperty(QString, status_message)]
        #[qproperty(QString, error_message)]
        #[qproperty(QString, username)]
        #[qproperty(QString, country)]
        #[qproperty(QString, language)]
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
        #[qproperty(QString, theme_mode)]
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
        #[qproperty(bool, credentials_encrypted)]
        #[qproperty(bool, remember_password)]
        #[qproperty(QString, credentials_vault_path)]
        #[qproperty(QString, event_log_json)]
        #[qproperty(i32, event_log_count)]
        #[qproperty(QString, app_version)]
        #[qproperty(QString, reading_age_display)]
        #[qproperty(bool, reading_stale)]
        #[qproperty(bool, alerts_snoozed)]
        #[qproperty(QString, snooze_until_display)]
        #[qproperty(f64, urgent_low_mg_dl)]
        #[qproperty(i32, alert_repeat_minutes)]
        #[qproperty(i32, stale_after_minutes)]
        #[qproperty(bool, alert_on_stale)]
        #[qproperty(bool, update_available)]
        #[qproperty(QString, latest_version)]
        #[qproperty(QString, update_url)]
        #[qproperty(QString, update_download_url)]
        #[qproperty(QString, update_status_message)]
        #[qproperty(bool, checking_for_updates)]
        #[qproperty(bool, installing_update)]
        #[qproperty(bool, check_updates_on_startup)]
        type AppController = super::AppControllerRust;

        #[qinvokable]
        fn initialize_controller(self: Pin<&mut AppController>);

        #[qinvokable]
        fn login(
            self: Pin<&mut AppController>,
            username: QString,
            password: QString,
            country: QString,
            language: QString,
        ) -> bool;

        #[qinvokable]
        fn logout(self: Pin<&mut AppController>);

        #[qinvokable]
        fn refresh_glucose_now(self: Pin<&mut AppController>);

        #[qinvokable]
        fn update_theme_mode(self: Pin<&mut AppController>, mode: QString);

        #[qinvokable]
        fn update_result_unit(self: Pin<&mut AppController>, unit: QString);

        #[qinvokable]
        fn update_country(self: Pin<&mut AppController>, country: QString);

        #[qinvokable]
        fn update_remember_password(self: Pin<&mut AppController>, enabled: bool);

        #[qinvokable]
        fn update_window_mode(self: Pin<&mut AppController>, mode: QString);

        #[qinvokable]
        fn update_overlay_visible(self: Pin<&mut AppController>, visible: bool);

        #[qinvokable]
        fn update_overlay_only_mode(self: Pin<&mut AppController>, only_overlay: bool);

        #[qinvokable]
        fn update_overlay_on_all_desktops(self: Pin<&mut AppController>, enabled: bool);

        #[qinvokable]
        fn update_overlay_dark_theme(self: Pin<&mut AppController>, enabled: bool);

        #[qinvokable]
        fn update_overlay_opacity_percent(self: Pin<&mut AppController>, opacity_percent: i32);

        #[qinvokable]
        fn save_overlay_placement(
            self: Pin<&mut AppController>,
            screen_name: QString,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        );

        #[qinvokable]
        fn begin_overlay_drag(self: Pin<&mut AppController>);

        #[qinvokable]
        fn update_overlay_drag(self: Pin<&mut AppController>);

        #[qinvokable]
        fn end_overlay_drag(self: Pin<&mut AppController>);

        #[qinvokable]
        fn popup_tray_menu_at_cursor(self: Pin<&mut AppController>);

        #[qinvokable]
        fn update_show_glucose_in_tray_icon(self: Pin<&mut AppController>, enabled: bool);

        #[qinvokable]
        fn update_show_glucose_in_tray_tooltip(self: Pin<&mut AppController>, enabled: bool);

        #[qinvokable]
        fn update_autostart_enabled(self: Pin<&mut AppController>, enabled: bool);

        #[qinvokable]
        fn update_start_minimized(self: Pin<&mut AppController>, enabled: bool);

        #[qinvokable]
        fn update_poll_interval_seconds(self: Pin<&mut AppController>, seconds: i32);

        #[qinvokable]
        fn save_alert_settings(
            self: Pin<&mut AppController>,
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
        fn test_alert_now(self: Pin<&mut AppController>);

        #[qinvokable]
        fn save_advanced_alert_settings(
            self: Pin<&mut AppController>,
            urgent_low: f64,
            repeat_minutes: i32,
            stale_after_minutes: i32,
            alert_on_stale: bool,
        );

        /// Snoozes non-urgent alerts for `minutes` (0 or negative cancels the snooze).
        #[qinvokable]
        fn snooze_alerts(self: Pin<&mut AppController>, minutes: i32);

        /// Handles an interactive desktop notification action (e.g. `"snooze_30"`, `"snooze_60"`,
        /// `"snooze_120"`, or `"default"`). Returns `true` if the action was recognized.
        #[qinvokable]
        fn handle_notification_action(self: Pin<&mut AppController>, action_key: QString) -> bool;

        /// Periodic tick (QML timer): refreshes reading age/staleness and evaluates alerts,
        /// so signal loss is detected even when fetches keep failing.
        #[qinvokable]
        fn evaluate_staleness(self: Pin<&mut AppController>);

        #[qinvokable]
        fn load_demo_sample_data(self: Pin<&mut AppController>);

        #[qinvokable]
        fn refresh_event_logs(self: Pin<&mut AppController>);

        #[qinvokable]
        fn clear_event_logs(self: Pin<&mut AppController>);

        #[qinvokable]
        fn export_event_logs(self: Pin<&mut AppController>, path: QString) -> bool;

        #[qinvokable]
        fn show_main_window(self: Pin<&mut AppController>);

        #[qinvokable]
        fn hide_main_window(self: Pin<&mut AppController>);

        #[qinvokable]
        fn quit_application(self: Pin<&mut AppController>);

        #[qinvokable]
        fn check_for_updates(self: Pin<&mut AppController>);

        #[qinvokable]
        fn check_for_updates_silent(self: Pin<&mut AppController>);

        #[qinvokable]
        fn install_update(self: Pin<&mut AppController>);

        #[qinvokable]
        fn update_check_updates_on_startup(self: Pin<&mut AppController>, enabled: bool);
    }

    // Lets background tokio tasks queue result handlers back onto the Qt GUI thread.
    impl cxx_qt::Threading for AppController {}
}

use std::path::Path;
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use crate::api::{authenticate_async, normalize_country_code, ApiError, AuthSuccess};
use crate::bridge::autostart::{
    background_runtime, is_flatpak, now_epoch, sync_autostart_desktop_file,
};
use crate::bridge::settings_handlers::current_settings_from_controller;
use crate::crypto::{
    clear_default_credentials, load_default_credentials, save_default_credentials,
    StoredCredentials,
};
use crate::event_log::{
    clear_events, export_events_to_path, log_error, log_info, log_warn, mask_email,
};
use crate::models::{
    clamp_poll_interval, format_glucose_value, format_target_range, load_default_settings,
    save_default_settings, trend_arrow_symbol, AlertKind, GlucoseSnapshot,
    DEFAULT_TARGET_HIGH_MG_DL, DEFAULT_TARGET_LOW_MG_DL,
};
use crate::paths::credentials_file_path;
use crate::session::{
    build_demo_snapshot, fetch_snapshot_with_reauth_async, format_snooze_until, RefreshOutcome,
};
use crate::updater::GITHUB_RELEASES_PAGE_URL;

pub struct AppControllerRust {
    logged_in: bool,
    busy: bool,
    status_message: QString,
    error_message: QString,
    username: QString,
    country: QString,
    language: QString,
    patient_name: QString,
    patient_id: QString,
    glucose_display: QString,
    glucose_mg_dl: f64,
    result_unit: QString,
    trend_arrow: i32,
    trend_symbol: QString,
    trend_description: QString,
    range_status_label: QString,
    range_color_hex: QString,
    target_range_display: QString,
    target_low_mg_dl: f64,
    target_high_mg_dl: f64,
    last_updated: QString,
    graph_points_json: QString,
    theme_mode: QString,
    window_mode: QString,
    overlay_visible: bool,
    overlay_only_mode: bool,
    overlay_on_all_desktops: bool,
    overlay_dark_theme: bool,
    overlay_opacity_percent: i32,
    overlay_screen_name: QString,
    overlay_x: i32,
    overlay_y: i32,
    overlay_width: i32,
    overlay_height: i32,
    show_glucose_in_tray_icon: bool,
    show_glucose_in_tray_tooltip: bool,
    autostart_enabled: bool,
    start_minimized: bool,
    poll_interval_seconds: i32,
    bring_to_front_enabled: bool,
    flash_window_enabled: bool,
    audio_alert_enabled: bool,
    use_custom_sound: bool,
    custom_sound_path: QString,
    override_threshold: bool,
    custom_target_low: f64,
    custom_target_high: f64,
    credentials_encrypted: bool,
    remember_password: bool,
    credentials_vault_path: QString,
    event_log_json: QString,
    event_log_count: i32,
    app_version: QString,
    reading_age_display: QString,
    reading_stale: bool,
    alerts_snoozed: bool,
    snooze_until_display: QString,
    urgent_low_mg_dl: f64,
    alert_repeat_minutes: i32,
    stale_after_minutes: i32,
    alert_on_stale: bool,
    update_available: bool,
    latest_version: QString,
    update_url: QString,
    update_download_url: QString,
    update_status_message: QString,
    checking_for_updates: bool,
    installing_update: bool,
    check_updates_on_startup: bool,
    pub(crate) last_notified_update_version: String,
    pub(crate) last_update_check_epoch: i64,
    /// Non-urgent alerts silenced until this epoch second (persisted in settings).
    pub(crate) snooze_until_epoch: i64,
    /// Last alert fired: (kind, epoch seconds). Reset when glucose returns to normal.
    pub(crate) last_alert: Option<(AlertKind, i64)>,
    pub(crate) snapshot: GlucoseSnapshot,
    pub(crate) creds: Option<StoredCredentials>,
    /// Sequence id of the running background CGM refresh (prevents overlapping polls).
    active_refresh: Option<u64>,
    refresh_seq: u64,
    /// A background login is running.
    login_in_flight: bool,
    /// Bumped on login/logout; background results tagged with an older value are dropped.
    session_generation: u64,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        let settings = load_default_settings();
        let initial_overlay_visible = settings.overlay_visible
            || settings.window_mode == "overlay"
            || settings.window_mode == "overlayTransparent";
        Self {
            logged_in: false,
            busy: false,
            status_message: QString::from("Ready"),
            error_message: QString::from(""),
            username: QString::from(""),
            country: QString::from(normalize_country_code(&settings.country).as_str()),
            language: QString::from(settings.language.as_str()),
            patient_name: QString::from(""),
            patient_id: QString::from(""),
            glucose_display: QString::from("--"),
            glucose_mg_dl: 0.0,
            result_unit: QString::from(settings.result_unit.as_str()),
            trend_arrow: 3,
            trend_symbol: QString::from("→"),
            trend_description: QString::from("Stable"),
            range_status_label: QString::from("Waiting"),
            range_color_hex: QString::from("#22c55e"),
            target_range_display: QString::from(format_target_range(
                DEFAULT_TARGET_LOW_MG_DL,
                DEFAULT_TARGET_HIGH_MG_DL,
                &settings.result_unit,
            )),
            target_low_mg_dl: DEFAULT_TARGET_LOW_MG_DL,
            target_high_mg_dl: DEFAULT_TARGET_HIGH_MG_DL,
            last_updated: QString::from("Never"),
            graph_points_json: QString::from("[]"),
            theme_mode: QString::from(settings.theme_mode.as_str()),
            window_mode: QString::from(settings.window_mode.as_str()),
            overlay_visible: initial_overlay_visible,
            overlay_only_mode: settings.overlay_only_mode,
            overlay_on_all_desktops: settings.overlay_on_all_desktops,
            overlay_dark_theme: settings.overlay_dark_theme,
            overlay_opacity_percent: settings.overlay_opacity_percent.clamp(20, 100),
            overlay_screen_name: QString::from(settings.overlay_screen_name.as_str()),
            overlay_x: settings.overlay_x,
            overlay_y: settings.overlay_y,
            overlay_width: settings.overlay_width.clamp(230, 600),
            overlay_height: settings.overlay_height.clamp(80, 260),
            show_glucose_in_tray_icon: settings.show_glucose_in_tray_icon,
            show_glucose_in_tray_tooltip: settings.show_glucose_in_tray_tooltip,
            autostart_enabled: settings.autostart_enabled,
            start_minimized: settings.start_minimized,
            poll_interval_seconds: clamp_poll_interval(settings.poll_interval_seconds) as i32,
            bring_to_front_enabled: settings.bring_to_front_enabled,
            flash_window_enabled: settings.flash_window_enabled,
            audio_alert_enabled: settings.audio_alert_enabled,
            use_custom_sound: settings.use_custom_sound,
            custom_sound_path: QString::from(settings.custom_sound_path.as_str()),
            override_threshold: settings.override_threshold,
            custom_target_low: settings.custom_target_low,
            custom_target_high: settings.custom_target_high,
            credentials_encrypted: credentials_file_path().exists(),
            remember_password: settings.remember_password,
            credentials_vault_path: QString::from(
                credentials_file_path().to_string_lossy().as_ref(),
            ),
            event_log_json: QString::from("[]"),
            event_log_count: 0,
            app_version: QString::from(env!("CARGO_PKG_VERSION")),
            reading_age_display: QString::from(""),
            reading_stale: false,
            alerts_snoozed: settings.is_snoozed(chrono::Utc::now().timestamp()),
            snooze_until_display: QString::from(
                format_snooze_until(settings.snooze_until_epoch, chrono::Utc::now().timestamp())
                    .as_str(),
            ),
            urgent_low_mg_dl: settings.effective_urgent_low(),
            alert_repeat_minutes: settings.alert_repeat_minutes.clamp(1, 240) as i32,
            stale_after_minutes: settings.effective_stale_after_minutes() as i32,
            alert_on_stale: settings.alert_on_stale,
            update_available: false,
            latest_version: QString::from(format!("v{}", env!("CARGO_PKG_VERSION")).as_str()),
            update_url: QString::from(GITHUB_RELEASES_PAGE_URL),
            update_download_url: QString::from(GITHUB_RELEASES_PAGE_URL),
            update_status_message: QString::from(
                format!("Current version: v{}", env!("CARGO_PKG_VERSION")).as_str(),
            ),
            checking_for_updates: false,
            installing_update: false,
            check_updates_on_startup: settings.check_updates_on_startup,
            last_notified_update_version: String::new(),
            last_update_check_epoch: 0,
            snooze_until_epoch: settings.snooze_until_epoch,
            last_alert: None,
            snapshot: GlucoseSnapshot::default(),
            creds: None,
            active_refresh: None,
            refresh_seq: 0,
            login_in_flight: false,
            session_generation: 0,
        }
    }
}

impl qobject::AppController {
    pub fn initialize_controller(mut self: Pin<&mut Self>) {
        log_info(
            "SYSTEM",
            format!(
                "Initializing LibreLinkUp Desktop v{} (KDE Plasma 6 / Kirigami)",
                env!("CARGO_PKG_VERSION")
            ),
        );

        let theme = self.theme_mode().clone();
        qobject::librelinkup_apply_app_theme(&theme);

        let ov_screen = self.overlay_screen_name().clone();
        let ov_x = *self.overlay_x();
        let ov_y = *self.overlay_y();
        let ov_w = *self.overlay_width();
        let ov_h = *self.overlay_height();
        qobject::librelinkup_restore_overlay_placement(&ov_screen, ov_x, ov_y, ov_w, ov_h);
        qobject::librelinkup_apply_overlay_all_desktops(*self.overlay_on_all_desktops());

        // Inside Flatpak, (re)register login autostart with the Background portal so the
        // entry exists and carries the current --minimized preference.
        if is_flatpak() && *self.autostart_enabled() {
            sync_autostart_desktop_file(true, *self.start_minimized());
        }

        match load_default_credentials() {
            Ok(Some(mut creds)) if creds.has_valid_session() => {
                if !creds.country.is_empty() {
                    let norm_country = normalize_country_code(&creds.country);
                    if norm_country != creds.country {
                        log_info(
                            "API",
                            format!(
                                "Normalized stored country code '{}' to regional endpoint '{}'.",
                                creds.country, norm_country
                            ),
                        );
                        creds.country = norm_country;
                        let remember_pw = *self.remember_password();
                        let _ =
                            save_default_credentials(&creds.sanitized_for_storage(remember_pw));
                    }
                }
                log_info(
                    "CRYPTO",
                    format!(
                        "Loaded AES-256-GCM encrypted credentials for region '{}' (account_id present).",
                        creds.country
                    ),
                );
                self.as_mut().set_logged_in(true);
                self.as_mut().set_credentials_encrypted(true);
                self.as_mut()
                    .set_username(QString::from(creds.username.as_str()));
                if !creds.country.is_empty() {
                    self.as_mut()
                        .set_country(QString::from(creds.country.as_str()));
                    let settings = current_settings_from_controller(&self);
                    let _ = save_default_settings(&settings);
                }
                if !creds.language.is_empty() {
                    self.as_mut()
                        .set_language(QString::from(creds.language.as_str()));
                }
                let full_name = format!("{} {}", creds.first_name.trim(), creds.last_name.trim())
                    .trim()
                    .to_string();
                if !full_name.is_empty() {
                    self.as_mut()
                        .set_patient_name(QString::from(full_name.as_str()));
                }
                self.as_mut().rust_mut().creds = Some(creds);
                self.as_mut()
                    .set_status_message(QString::from("Authenticated (AES-256-GCM Vault)"));
                self.as_mut().refresh_glucose_now();
            }
            Ok(_) => {
                log_info(
                    "AUTH",
                    "No stored session found in encrypted vault — awaiting login.",
                );
                self.as_mut()
                    .set_status_message(QString::from("Please sign in to LibreLinkUp"));
            }
            Err(e) => {
                log_warn(
                    "CRYPTO",
                    format!("Could not decrypt stored credentials: {e}"),
                );
                self.as_mut().set_error_message(QString::from(e.as_str()));
            }
        }

        if *self.overlay_visible() && *self.overlay_only_mode() {
            qobject::librelinkup_hide_main_window();
        }

        if *self.check_updates_on_startup()
            && std::env::var_os("QT_QPA_PLATFORM").as_deref()
                != Some(std::ffi::OsStr::new("offscreen"))
        {
            self.as_mut().start_update_check(false);
        }

        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    fn update_busy_flag(mut self: Pin<&mut Self>) {
        let busy = self.rust().login_in_flight || self.rust().active_refresh.is_some();
        self.as_mut().set_busy(busy);
    }

    /// Starts an asynchronous LibreLinkUp login. Returns `true` when the request was
    /// started; the outcome is reported through `loggedIn`, `errorMessage` and
    /// `statusMessage` once the background task completes (the GUI never blocks).
    pub fn login(
        mut self: Pin<&mut Self>,
        username: QString,
        password: QString,
        country: QString,
        language: QString,
    ) -> bool {
        let user_str = username.to_string().trim().to_string();
        let pass_str = password.to_string();
        let country_str = if country.to_string().trim().is_empty() {
            "us".to_string()
        } else {
            normalize_country_code(&country.to_string())
        };
        let lang_str = if language.to_string().trim().is_empty() {
            "en".to_string()
        } else {
            language.to_string().trim().to_lowercase()
        };

        if self.rust().login_in_flight {
            return false;
        }

        if user_str.len() < 2 || pass_str.len() < 2 {
            let msg = "Please enter both your LibreLinkUp email and password.";
            self.as_mut().set_error_message(QString::from(msg));
            log_warn("AUTH", msg);
            self.as_mut().sync_logs_to_properties();
            return false;
        }

        let Some(runtime) = background_runtime() else {
            let msg = "Failed to initialize async runtime.";
            self.as_mut().set_error_message(QString::from(msg));
            log_error("SYSTEM", msg);
            self.as_mut().sync_logs_to_properties();
            return false;
        };

        // A new session supersedes any refresh still running for the previous one.
        let generation = self.rust().session_generation.wrapping_add(1);
        self.as_mut().rust_mut().session_generation = generation;
        self.as_mut().rust_mut().active_refresh = None;
        self.as_mut().rust_mut().login_in_flight = true;
        self.as_mut().update_busy_flag();
        self.as_mut().set_error_message(QString::from(""));
        self.as_mut()
            .set_status_message(QString::from("Signing in to LibreLinkUp..."));
        log_info(
            "AUTH",
            format!(
                "Attempting LibreLinkUp login for '{}' (region: {country_str})",
                mask_email(&user_str)
            ),
        );
        self.as_mut().sync_logs_to_properties();

        let qt_thread = self.qt_thread();
        runtime.spawn(async move {
            let auth_res = authenticate_async(&country_str, &user_str, &pass_str).await;
            let queued = qt_thread.queue(move |qobject| {
                qobject.apply_login_outcome(generation, user_str, pass_str, lang_str, auth_res);
            });
            if queued.is_err() {
                log_warn(
                    "SYSTEM",
                    "Login finished after the controller was destroyed.",
                );
            }
        });
        true
    }

    fn apply_login_outcome(
        mut self: Pin<&mut Self>,
        generation: u64,
        user_str: String,
        pass_str: String,
        lang_str: String,
        auth_res: Result<AuthSuccess, ApiError>,
    ) {
        if generation != self.rust().session_generation {
            // Superseded (e.g. user signed out while the request was running).
            self.as_mut().update_busy_flag();
            return;
        }
        self.as_mut().rust_mut().login_in_flight = false;
        self.as_mut().update_busy_flag();

        match auth_res {
            Ok(auth) => {
                let stored = StoredCredentials {
                    username: user_str.clone(),
                    password: pass_str,
                    token: auth.token.clone(),
                    account_id: auth.account_id.clone(),
                    country: auth.country.clone(),
                    language: lang_str.clone(),
                    patient_id: String::new(),
                    first_name: auth.first_name.clone(),
                    last_name: auth.last_name.clone(),
                };
                let remember_pw = *self.remember_password();
                match save_default_credentials(&stored.sanitized_for_storage(remember_pw)) {
                    Ok(()) => {
                        log_info(
                            "CRYPTO",
                            "Encrypted and saved session credentials with AES-256-GCM + PBKDF2-HMAC-SHA256.",
                        );
                        self.as_mut().set_credentials_encrypted(true);
                    }
                    Err(e) => {
                        log_error(
                            "CRYPTO",
                            format!("Failed to save encrypted credentials: {e}"),
                        );
                    }
                }

                self.as_mut().set_username(QString::from(user_str.as_str()));
                self.as_mut()
                    .set_country(QString::from(auth.country.as_str()));
                self.as_mut().set_language(QString::from(lang_str.as_str()));
                self.as_mut().set_logged_in(true);
                self.as_mut().rust_mut().creds = Some(stored);
                let settings = current_settings_from_controller(&self);
                let _ = save_default_settings(&settings);

                log_info(
                    "AUTH",
                    format!("Login succeeded (resolved region: '{}').", auth.country),
                );
                self.as_mut()
                    .set_status_message(QString::from("Signed in — fetching glucose data..."));
                self.as_mut().refresh_glucose_now();
                self.as_mut().sync_logs_to_properties();
            }
            Err(err) => {
                let msg = err.to_string();
                log_error("AUTH", format!("Login failed: {msg}"));
                self.as_mut().set_error_message(QString::from(msg.as_str()));
                self.as_mut()
                    .set_status_message(QString::from("Authentication failed"));
                self.as_mut().sync_logs_to_properties();
            }
        }
    }

    pub fn logout(mut self: Pin<&mut Self>) {
        let _ = clear_default_credentials();
        // Invalidate any in-flight login/refresh so their results are ignored.
        let generation = self.rust().session_generation.wrapping_add(1);
        self.as_mut().rust_mut().session_generation = generation;
        self.as_mut().rust_mut().login_in_flight = false;
        self.as_mut().rust_mut().active_refresh = None;
        self.as_mut().update_busy_flag();
        self.as_mut().rust_mut().creds = None;
        self.as_mut().rust_mut().snapshot = GlucoseSnapshot::default();
        self.as_mut().rust_mut().last_alert = None;
        self.as_mut().set_logged_in(false);
        self.as_mut().set_credentials_encrypted(false);
        self.as_mut().set_patient_name(QString::from(""));
        self.as_mut().set_patient_id(QString::from(""));
        self.as_mut().set_error_message(QString::from(""));
        self.as_mut()
            .set_status_message(QString::from("Signed out — encrypted vault cleared."));
        log_info(
            "AUTH",
            "User signed out and encrypted credentials file deleted.",
        );
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    /// Starts an asynchronous CGM refresh (no-op if one is already running).
    pub fn refresh_glucose_now(mut self: Pin<&mut Self>) {
        if self.rust().active_refresh.is_some() || self.rust().login_in_flight {
            return;
        }
        let Some(creds) = self.rust().creds.clone() else {
            return;
        };
        if !creds.has_valid_session() {
            return;
        }
        let Some(runtime) = background_runtime() else {
            self.as_mut()
                .set_error_message(QString::from("Failed to initialize async runtime."));
            return;
        };

        let generation = self.rust().session_generation;
        let seq = self.rust().refresh_seq.wrapping_add(1);
        self.as_mut().rust_mut().refresh_seq = seq;
        self.as_mut().rust_mut().active_refresh = Some(seq);
        self.as_mut().update_busy_flag();
        self.as_mut().set_error_message(QString::from(""));

        let qt_thread = self.qt_thread();
        runtime.spawn(async move {
            let outcome = fetch_snapshot_with_reauth_async(&creds).await;
            let queued = qt_thread.queue(move |qobject| {
                qobject.apply_refresh_outcome(generation, seq, outcome);
            });
            if queued.is_err() {
                log_warn(
                    "SYSTEM",
                    "Refresh finished after the controller was destroyed.",
                );
            }
        });
    }

    fn apply_refresh_outcome(
        mut self: Pin<&mut Self>,
        generation: u64,
        seq: u64,
        outcome: Result<RefreshOutcome, ApiError>,
    ) {
        if self.rust().active_refresh == Some(seq) {
            self.as_mut().rust_mut().active_refresh = None;
        }
        self.as_mut().update_busy_flag();
        if generation != self.rust().session_generation {
            // Session changed (logout/re-login) while this refresh was running: drop it.
            return;
        }

        match outcome {
            Ok(outcome) => {
                if let Some(renewed) = outcome.updated_credentials {
                    // Persist on the Qt thread, and only for the still-current session, so a
                    // logout during the request can never resurrect the deleted vault.
                    let remember_pw = *self.remember_password();
                    if let Err(e) =
                        save_default_credentials(&renewed.sanitized_for_storage(remember_pw))
                    {
                        log_error("CRYPTO", format!("Failed to save renewed credentials: {e}"));
                    }
                    self.as_mut().rust_mut().creds = Some(renewed);
                }

                let snap = outcome.snapshot;
                let unit = self.result_unit().to_string();
                let formatted = format_glucose_value(snap.value_mg_dl, &unit);
                let arrow = trend_arrow_symbol(snap.trend_arrow);
                log_info(
                    "GLUCOSE",
                    format!(
                        "Fetched glucose reading: {} {} {} ({} points)",
                        formatted,
                        unit,
                        arrow,
                        snap.graph_points.len()
                    ),
                );

                self.as_mut().rust_mut().snapshot = snap;
                self.as_mut()
                    .set_status_message(QString::from("Live CGM telemetry active"));
            }
            Err(err) => {
                let msg = err.to_string();
                log_warn("API", format!("Failed to fetch CGM data: {msg}"));
                self.as_mut().set_error_message(QString::from(msg.as_str()));
            }
        }

        // Evaluate after both success and failure: a failing fetch can still mean the
        // last reading has gone stale.
        self.as_mut().sync_tray_and_ui();
        self.as_mut().evaluate_alerts();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn load_demo_sample_data(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().snapshot = build_demo_snapshot(now_epoch());
        self.as_mut().set_logged_in(true);
        self.as_mut()
            .set_status_message(QString::from("Sample CGM telemetry loaded"));
        log_info(
            "GLUCOSE",
            "Loaded sample CGM telemetry (112 mg/dL → Stable).",
        );
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn refresh_event_logs(mut self: Pin<&mut Self>) {
        self.as_mut().sync_logs_to_properties();
    }

    pub fn clear_event_logs(mut self: Pin<&mut Self>) {
        clear_events();
        log_info("SYSTEM", "Event log cleared by user.");
        self.as_mut().sync_logs_to_properties();
    }

    pub fn export_event_logs(mut self: Pin<&mut Self>, path: QString) -> bool {
        let p = path.to_string();
        match export_events_to_path(Path::new(&p)) {
            Ok(n) => {
                log_info("SYSTEM", format!("Exported {n} log entries to {p}"));
                self.as_mut().sync_logs_to_properties();
                true
            }
            Err(e) => {
                log_error("SYSTEM", format!("Failed to export logs: {e}"));
                self.as_mut().sync_logs_to_properties();
                false
            }
        }
    }

    pub fn show_main_window(mut self: Pin<&mut Self>) {
        self.as_mut().set_overlay_only_mode(false);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        qobject::librelinkup_show_main_window();
        // If auto-check is enabled and it has been > 1 hour since the last check, check in the background
        if *self.check_updates_on_startup()
            && std::env::var_os("QT_QPA_PLATFORM").as_deref()
                != Some(std::ffi::OsStr::new("offscreen"))
            && (now_epoch() - self.rust().last_update_check_epoch >= 3600)
        {
            self.as_mut().start_update_check(false);
        }
        self.as_mut().sync_tray_and_ui();
    }

    pub fn hide_main_window(mut self: Pin<&mut Self>) {
        let only_ov = *self.overlay_visible();
        self.as_mut().set_overlay_only_mode(only_ov);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        qobject::librelinkup_hide_main_window();
        self.as_mut().sync_tray_and_ui();
    }

    pub fn quit_application(self: Pin<&mut Self>) {
        qobject::librelinkup_quit_app();
    }
}
