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
        fn librelinkup_trigger_window_alert(bring_to_front: bool, flash_window: bool);
        fn librelinkup_play_alert_sound(custom_sound_path: &QString);
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
        #[qproperty(bool, overlay_dark_theme)]
        #[qproperty(i32, overlay_opacity_percent)]
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
        #[qproperty(QString, credentials_vault_path)]
        #[qproperty(QString, event_log_json)]
        #[qproperty(i32, event_log_count)]
        #[qproperty(QString, app_version)]
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
        fn update_window_mode(self: Pin<&mut AppController>, mode: QString);

        #[qinvokable]
        fn update_overlay_visible(self: Pin<&mut AppController>, visible: bool);

        #[qinvokable]
        fn update_overlay_dark_theme(self: Pin<&mut AppController>, enabled: bool);

        #[qinvokable]
        fn update_overlay_opacity_percent(self: Pin<&mut AppController>, opacity_percent: i32);

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
    }
}

use std::fs;
use std::path::Path;
use std::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::api::{authenticate_async, fetch_cgm_snapshot_async};
use crate::crypto::{
    clear_default_credentials, load_default_credentials, save_default_credentials,
    StoredCredentials,
};
use crate::event_log::{
    clear_events, export_events_to_path, get_event_count, get_events_json, log_error, log_info,
    log_warn,
};
use crate::models::{
    format_glucose_value, format_target_range, load_default_settings, save_default_settings,
    trend_arrow_description, trend_arrow_symbol, AppSettings, GlucoseRangeStatus, GlucoseSnapshot,
    GraphPoint, DEFAULT_TARGET_HIGH_MG_DL, DEFAULT_TARGET_LOW_MG_DL,
};
use crate::paths::{autostart_desktop_path, credentials_file_path, APP_ID};

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
    overlay_dark_theme: bool,
    overlay_opacity_percent: i32,
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
    credentials_vault_path: QString,
    event_log_json: QString,
    event_log_count: i32,
    app_version: QString,
    snapshot: GlucoseSnapshot,
    creds: Option<StoredCredentials>,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        let settings = load_default_settings();
        Self {
            logged_in: false,
            busy: false,
            status_message: QString::from("Ready"),
            error_message: QString::from(""),
            username: QString::from(""),
            country: QString::from(settings.country.as_str()),
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
            overlay_visible: settings.window_mode == "overlay"
                || settings.window_mode == "overlayTransparent",
            overlay_dark_theme: settings.overlay_dark_theme,
            overlay_opacity_percent: settings.overlay_opacity_percent.clamp(20, 100),
            show_glucose_in_tray_icon: settings.show_glucose_in_tray_icon,
            show_glucose_in_tray_tooltip: settings.show_glucose_in_tray_tooltip,
            autostart_enabled: settings.autostart_enabled,
            start_minimized: settings.start_minimized,
            poll_interval_seconds: settings.poll_interval_seconds as i32,
            bring_to_front_enabled: settings.bring_to_front_enabled,
            flash_window_enabled: settings.flash_window_enabled,
            audio_alert_enabled: settings.audio_alert_enabled,
            use_custom_sound: settings.use_custom_sound,
            custom_sound_path: QString::from(settings.custom_sound_path.as_str()),
            override_threshold: settings.override_threshold,
            custom_target_low: settings.custom_target_low,
            custom_target_high: settings.custom_target_high,
            credentials_encrypted: credentials_file_path().exists(),
            credentials_vault_path: QString::from(
                credentials_file_path().to_string_lossy().as_ref(),
            ),
            event_log_json: QString::from("[]"),
            event_log_count: 0,
            app_version: QString::from(env!("CARGO_PKG_VERSION")),
            snapshot: GlucoseSnapshot::default(),
            creds: None,
        }
    }
}

fn current_settings_from_controller(ctrl: &qobject::AppController) -> AppSettings {
    AppSettings {
        theme_mode: ctrl.theme_mode().to_string(),
        result_unit: ctrl.result_unit().to_string(),
        country: ctrl.country().to_string(),
        language: ctrl.language().to_string(),
        window_mode: ctrl.window_mode().to_string(),
        overlay_dark_theme: *ctrl.overlay_dark_theme(),
        overlay_opacity_percent: (*ctrl.overlay_opacity_percent()).clamp(20, 100),
        show_glucose_in_tray_icon: *ctrl.show_glucose_in_tray_icon(),
        show_glucose_in_tray_tooltip: *ctrl.show_glucose_in_tray_tooltip(),
        tray_icon_theme: "auto".to_string(),
        autostart_enabled: *ctrl.autostart_enabled(),
        start_minimized: *ctrl.start_minimized(),
        poll_interval_seconds: (*ctrl.poll_interval_seconds()).clamp(15, 3600) as u32,
        bring_to_front_enabled: *ctrl.bring_to_front_enabled(),
        flash_window_enabled: *ctrl.flash_window_enabled(),
        audio_alert_enabled: *ctrl.audio_alert_enabled(),
        use_custom_sound: *ctrl.use_custom_sound(),
        custom_sound_path: ctrl.custom_sound_path().to_string(),
        override_threshold: *ctrl.override_threshold(),
        custom_target_low: *ctrl.custom_target_low(),
        custom_target_high: *ctrl.custom_target_high(),
        ui_text_scale: 100,
    }
}

fn sync_autostart_desktop_file(enabled: bool, start_minimized: bool) {
    let desktop_path = autostart_desktop_path();
    if !enabled {
        if desktop_path.exists() {
            let _ = fs::remove_file(&desktop_path);
            log_info("SYSTEM", "Removed KDE autostart desktop entry.");
        }
        return;
    }

    let flag = if start_minimized { " --minimized" } else { "" };
    // Check if Flatpak is installed; otherwise use current executable path
    let exec_cmd = if Path::new("/.flatpak-info").exists() {
        format!("/usr/bin/flatpak run {APP_ID}{flag}")
    } else if let Ok(exe) = std::env::current_exe() {
        format!("{}{flag}", exe.to_string_lossy())
    } else {
        format!("librelinkup-kde{flag}")
    };

    let content = format!(
        "[Desktop Entry]\nType=Application\nName=LibreLinkUp Rust Desktop\nComment=KDE Plasma 6 Blood Glucose Monitor for LibreLinkUp\nExec={exec_cmd}\nIcon={APP_ID}\nTerminal=false\nStartupNotify=false\nX-KDE-autostart-after=panel\n"
    );
    if fs::write(&desktop_path, content).is_ok() {
        log_info(
            "SYSTEM",
            format!("Updated autostart entry at {}", desktop_path.display()),
        );
    }
}

impl qobject::AppController {
    fn sync_logs_to_properties(mut self: Pin<&mut Self>) {
        let json = get_events_json();
        let count = get_event_count() as i32;
        self.as_mut().set_event_log_json(QString::from(json.as_str()));
        self.as_mut().set_event_log_count(count);
    }

    fn sync_tray_and_ui(mut self: Pin<&mut Self>) {
        let settings = current_settings_from_controller(&self);
        let snapshot = self.rust().snapshot.clone();

        let eff_low = settings.effective_target_low(snapshot.target_low_mg_dl);
        let eff_high = settings.effective_target_high(snapshot.target_high_mg_dl);
        let val_display = format_glucose_value(snapshot.value_mg_dl, &settings.result_unit);
        let arrow_sym = trend_arrow_symbol(snapshot.trend_arrow);
        let arrow_desc = trend_arrow_description(snapshot.trend_arrow);
        let range_str = format_target_range(eff_low, eff_high, &settings.result_unit);

        let (status_label, color_hex) = if snapshot.value_mg_dl > 0.0 {
            let st = GlucoseRangeStatus::classify(snapshot.value_mg_dl, eff_low, eff_high);
            (st.label(), st.color_hex())
        } else {
            ("Waiting", "#22c55e")
        };

        self.as_mut()
            .set_glucose_display(QString::from(val_display.as_str()));
        self.as_mut().set_glucose_mg_dl(snapshot.value_mg_dl);
        self.as_mut().set_trend_arrow(snapshot.trend_arrow);
        self.as_mut().set_trend_symbol(QString::from(arrow_sym));
        self.as_mut()
            .set_trend_description(QString::from(arrow_desc));
        self.as_mut()
            .set_range_status_label(QString::from(status_label));
        self.as_mut().set_range_color_hex(QString::from(color_hex));
        self.as_mut()
            .set_target_range_display(QString::from(range_str.as_str()));
        self.as_mut().set_target_low_mg_dl(eff_low);
        self.as_mut().set_target_high_mg_dl(eff_high);

        let full_name = format!(
            "{} {}",
            snapshot.first_name.trim(),
            snapshot.last_name.trim()
        )
        .trim()
        .to_string();
        if !full_name.is_empty() {
            self.as_mut()
                .set_patient_name(QString::from(full_name.as_str()));
        }
        if !snapshot.patient_id.is_empty() {
            self.as_mut()
                .set_patient_id(QString::from(snapshot.patient_id.as_str()));
        }
        if !snapshot.timestamp.is_empty() {
            self.as_mut()
                .set_last_updated(QString::from(snapshot.timestamp.as_str()));
        }

        let points_json =
            serde_json::to_string(&snapshot.graph_points).unwrap_or_else(|_| "[]".to_string());
        self.as_mut()
            .set_graph_points_json(QString::from(points_json.as_str()));

        let badge_text = settings.format_tray_icon_badge(&snapshot);
        let tooltip_text = settings.format_tray_hover_tooltip(&snapshot);
        let status_header = if snapshot.value_mg_dl > 0.0 {
            format!(
                "Blood Sugar: {} {} {} ({})",
                val_display, settings.result_unit, arrow_sym, arrow_desc
            )
        } else {
            "Blood Sugar: --".to_string()
        };
        let range_header = format!("Target Range: {range_str}");

        let use_light_on_dark = settings.theme_mode != "light";
        let mut flags: u8 = 0;
        if use_light_on_dark {
            flags |= 0x01;
        }
        if settings.show_glucose_in_tray_icon {
            flags |= 0x02;
        }
        if settings.show_glucose_in_tray_tooltip {
            flags |= 0x04;
        }
        if settings.autostart_enabled {
            flags |= 0x08;
        }
        if *self.overlay_visible() {
            flags |= 0x10;
        }

        let q_theme = QString::from(settings.theme_mode.as_str());
        let q_badge = QString::from(badge_text.as_str());
        let q_arrow = QString::from(arrow_sym);
        let q_color = QString::from(color_hex);
        let q_status = QString::from(status_header.as_str());
        let q_range = QString::from(range_header.as_str());
        let q_tooltip = QString::from(tooltip_text.as_str());

        qobject::librelinkup_sync_tray(
            self.as_mut(),
            &q_theme,
            &q_badge,
            &q_arrow,
            &q_color,
            &q_status,
            &q_range,
            &q_tooltip,
            flags,
        );
    }

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

        match load_default_credentials() {
            Ok(Some(creds)) if creds.has_valid_session() => {
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
                }
                if !creds.language.is_empty() {
                    self.as_mut()
                        .set_language(QString::from(creds.language.as_str()));
                }
                let full_name =
                    format!("{} {}", creds.first_name.trim(), creds.last_name.trim())
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
                log_warn("CRYPTO", format!("Could not decrypt stored credentials: {e}"));
                self.as_mut().set_error_message(QString::from(e.as_str()));
            }
        }

        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

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
            country.to_string().trim().to_lowercase()
        };
        let lang_str = if language.to_string().trim().is_empty() {
            "en".to_string()
        } else {
            language.to_string().trim().to_lowercase()
        };

        if user_str.len() < 2 || pass_str.len() < 2 {
            let msg = "Please enter both your LibreLinkUp email and password.";
            self.as_mut().set_error_message(QString::from(msg));
            log_warn("AUTH", msg);
            self.as_mut().sync_logs_to_properties();
            return false;
        }

        self.as_mut().set_busy(true);
        self.as_mut().set_error_message(QString::from(""));
        self.as_mut()
            .set_status_message(QString::from("Signing in to LibreLinkUp..."));
        log_info(
            "AUTH",
            format!("Attempting LibreLinkUp login for '{user_str}' (region: {country_str})"),
        );

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let auth_res = match rt {
            Ok(runtime) => runtime.block_on(authenticate_async(&country_str, &user_str, &pass_str)),
            Err(e) => {
                self.as_mut().set_busy(false);
                let msg = format!("Failed to initialize async runtime: {e}");
                self.as_mut().set_error_message(QString::from(msg.as_str()));
                log_error("SYSTEM", msg);
                self.as_mut().sync_logs_to_properties();
                return false;
            }
        };

        self.as_mut().set_busy(false);

        match auth_res {
            Ok(auth) => {
                let stored = StoredCredentials {
                    username: user_str.clone(),
                    password: pass_str,
                    token: auth.token,
                    account_id: auth.account_id,
                    country: auth.country.clone(),
                    language: lang_str.clone(),
                    patient_id: String::new(),
                    first_name: auth.first_name.clone(),
                    last_name: auth.last_name.clone(),
                };
                match save_default_credentials(&stored) {
                    Ok(()) => {
                        log_info(
                            "CRYPTO",
                            "Encrypted and saved session credentials with AES-256-GCM + PBKDF2-HMAC-SHA256.",
                        );
                        self.as_mut().set_credentials_encrypted(true);
                    }
                    Err(e) => {
                        log_error("CRYPTO", format!("Failed to save encrypted credentials: {e}"));
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
                self.as_mut().refresh_glucose_now();
                self.as_mut().sync_logs_to_properties();
                true
            }
            Err(err) => {
                let msg = err.to_string();
                log_error("AUTH", format!("Login failed: {msg}"));
                self.as_mut().set_error_message(QString::from(msg.as_str()));
                self.as_mut()
                    .set_status_message(QString::from("Authentication failed"));
                self.as_mut().sync_logs_to_properties();
                false
            }
        }
    }

    pub fn logout(mut self: Pin<&mut Self>) {
        let _ = clear_default_credentials();
        self.as_mut().rust_mut().creds = None;
        self.as_mut().rust_mut().snapshot = GlucoseSnapshot::default();
        self.as_mut().set_logged_in(false);
        self.as_mut().set_credentials_encrypted(false);
        self.as_mut().set_patient_name(QString::from(""));
        self.as_mut().set_patient_id(QString::from(""));
        self.as_mut().set_error_message(QString::from(""));
        self.as_mut()
            .set_status_message(QString::from("Signed out — encrypted vault cleared."));
        log_info("AUTH", "User signed out and encrypted credentials file deleted.");
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn refresh_glucose_now(mut self: Pin<&mut Self>) {
        let creds_opt = self.rust().creds.clone();
        let Some(mut creds) = creds_opt else {
            return;
        };
        if !creds.has_valid_session() {
            return;
        }

        self.as_mut().set_busy(true);
        self.as_mut().set_error_message(QString::from(""));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let Ok(runtime) = rt else {
            self.as_mut().set_busy(false);
            return;
        };

        let mut fetch_res = runtime.block_on(fetch_cgm_snapshot_async(
            &creds.country,
            &creds.token,
            &creds.account_id,
        ));

        // If token expired and we have encrypted username/password, automatically re-authenticate!
        if matches!(fetch_res, Err(crate::api::ApiError::Unauthorized))
            && !creds.username.is_empty()
            && !creds.password.is_empty()
        {
            log_info(
                "AUTH",
                "Auth token expired; re-authenticating using encrypted credentials...",
            );
            if let Ok(reauth) = runtime.block_on(authenticate_async(
                &creds.country,
                &creds.username,
                &creds.password,
            )) {
                creds.token = reauth.token;
                creds.account_id = reauth.account_id;
                creds.country = reauth.country;
                let _ = save_default_credentials(&creds);
                self.as_mut().rust_mut().creds = Some(creds.clone());
                fetch_res = runtime.block_on(fetch_cgm_snapshot_async(
                    &creds.country,
                    &creds.token,
                    &creds.account_id,
                ));
            }
        }

        self.as_mut().set_busy(false);

        match fetch_res {
            Ok(snap) => {
                let unit = self.result_unit().to_string();
                let formatted = format_glucose_value(snap.value_mg_dl, &unit);
                let arrow = trend_arrow_symbol(snap.trend_arrow);
                log_info(
                    "GLUCOSE",
                    format!(
                        "Fetched glucose reading: {} {} {} ({} points, patient: {} {})",
                        formatted,
                        unit,
                        arrow,
                        snap.graph_points.len(),
                        snap.first_name,
                        snap.last_name
                    ),
                );

                let settings = current_settings_from_controller(&self);
                let should_alert = settings.should_trigger_alert(
                    snap.value_mg_dl,
                    snap.target_low_mg_dl,
                    snap.target_high_mg_dl,
                );

                self.as_mut().rust_mut().snapshot = snap;
                self.as_mut()
                    .set_status_message(QString::from("Live CGM telemetry active"));

                if should_alert {
                    log_warn(
                        "ALERT",
                        format!(
                            "Glucose reading ({formatted} {unit}) is outside target threshold!"
                        ),
                    );
                    if settings.bring_to_front_enabled || settings.flash_window_enabled {
                        qobject::librelinkup_trigger_window_alert(
                            settings.bring_to_front_enabled,
                            settings.flash_window_enabled,
                        );
                    }
                    if settings.audio_alert_enabled {
                        let sound_path = if settings.use_custom_sound {
                            QString::from(settings.custom_sound_path.as_str())
                        } else {
                            QString::from("")
                        };
                        qobject::librelinkup_play_alert_sound(&sound_path);
                    }
                }
            }
            Err(err) => {
                let msg = err.to_string();
                log_warn("API", format!("Failed to fetch CGM data: {msg}"));
                self.as_mut().set_error_message(QString::from(msg.as_str()));
            }
        }

        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_theme_mode(mut self: Pin<&mut Self>, mode: QString) {
        let clean = mode.to_string().trim().to_lowercase();
        let valid = match clean.as_str() {
            "dark" | "light" | "system" => clean,
            _ => "system".to_string(),
        };
        let q_mode = QString::from(valid.as_str());
        self.as_mut().set_theme_mode(q_mode.clone());
        qobject::librelinkup_apply_app_theme(&q_mode);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info("SYSTEM", format!("Switched color theme mode to '{valid}'."));
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_result_unit(mut self: Pin<&mut Self>, unit: QString) {
        let clean = if unit.to_string().trim().eq_ignore_ascii_case("mmol/L") {
            "mmol/L"
        } else {
            "mg/dL"
        };
        self.as_mut().set_result_unit(QString::from(clean));
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info("GLUCOSE", format!("Changed display unit to '{clean}'."));
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_country(mut self: Pin<&mut Self>, country: QString) {
        let clean = country.to_string().trim().to_lowercase();
        if !clean.is_empty() {
            self.as_mut().set_country(QString::from(clean.as_str()));
            let settings = current_settings_from_controller(&self);
            let _ = save_default_settings(&settings);
            log_info("API", format!("Updated regional server to '{clean}'."));
            self.as_mut().sync_logs_to_properties();
        }
    }

    pub fn update_window_mode(mut self: Pin<&mut Self>, mode: QString) {
        let clean = mode.to_string().trim().to_string();
        let valid = match clean.as_str() {
            "overlay" | "overlayTransparent" | "windowed" => clean,
            _ => "windowed".to_string(),
        };
        let is_overlay = valid == "overlay" || valid == "overlayTransparent";
        self.as_mut().set_window_mode(QString::from(valid.as_str()));
        self.as_mut().set_overlay_visible(is_overlay);
        if valid == "overlayTransparent" && *self.overlay_opacity_percent() > 75 {
            self.as_mut().set_overlay_opacity_percent(55);
        } else if valid == "overlay" && *self.overlay_opacity_percent() < 75 {
            self.as_mut().set_overlay_opacity_percent(88);
        }
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info("SYSTEM", format!("Set window mode to '{valid}'."));
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_overlay_visible(mut self: Pin<&mut Self>, visible: bool) {
        self.as_mut().set_overlay_visible(visible);
        if visible && self.window_mode().to_string() == "windowed" {
            self.as_mut().set_window_mode(QString::from("overlay"));
        } else if !visible {
            self.as_mut().set_window_mode(QString::from("windowed"));
        }
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info(
            "SYSTEM",
            format!("Floating desktop overlay visible: {visible}"),
        );
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_overlay_dark_theme(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_overlay_dark_theme(enabled);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info(
            "SYSTEM",
            format!("Desktop overlay dark theme: {enabled}"),
        );
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_overlay_opacity_percent(mut self: Pin<&mut Self>, opacity_percent: i32) {
        let clamped = opacity_percent.clamp(20, 100);
        self.as_mut().set_overlay_opacity_percent(clamped);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info(
            "SYSTEM",
            format!("Desktop overlay opacity set to {clamped}%"),
        );
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_show_glucose_in_tray_icon(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_show_glucose_in_tray_icon(enabled);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info(
            "TRAY",
            format!("Show glucose level in tray icon badge: {enabled}"),
        );
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_show_glucose_in_tray_tooltip(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_show_glucose_in_tray_tooltip(enabled);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info(
            "TRAY",
            format!("Show glucose telemetry in tray hover tooltip: {enabled}"),
        );
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_autostart_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_autostart_enabled(enabled);
        let start_min = *self.start_minimized();
        sync_autostart_desktop_file(enabled, start_min);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_start_minimized(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_start_minimized(enabled);
        let autostart = *self.autostart_enabled();
        if autostart {
            sync_autostart_desktop_file(autostart, enabled);
        }
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info("SYSTEM", format!("Start minimized to tray on login: {enabled}"));
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_poll_interval_seconds(mut self: Pin<&mut Self>, seconds: i32) {
        let clamped = seconds.clamp(15, 3600);
        self.as_mut().set_poll_interval_seconds(clamped);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info(
            "API",
            format!("Updated polling interval to {clamped} seconds."),
        );
        self.as_mut().sync_logs_to_properties();
    }

    pub fn save_alert_settings(
        mut self: Pin<&mut Self>,
        bring_to_front: bool,
        flash_window: bool,
        audio_alert: bool,
        use_custom_sound: bool,
        custom_sound_path: QString,
        override_threshold: bool,
        custom_low: f64,
        custom_high: f64,
    ) {
        let clean_path = custom_sound_path
            .to_string()
            .trim()
            .trim_start_matches("file://")
            .to_string();
        let low = custom_low.clamp(40.0, 200.0);
        let high = custom_high.clamp(low + 10.0, 400.0);

        self.as_mut().set_bring_to_front_enabled(bring_to_front);
        self.as_mut().set_flash_window_enabled(flash_window);
        self.as_mut().set_audio_alert_enabled(audio_alert);
        self.as_mut().set_use_custom_sound(use_custom_sound);
        self.as_mut()
            .set_custom_sound_path(QString::from(clean_path.as_str()));
        self.as_mut().set_override_threshold(override_threshold);
        self.as_mut().set_custom_target_low(low);
        self.as_mut().set_custom_target_high(high);

        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info(
            "ALERT",
            format!(
                "Saved alert settings (override={override_threshold}, range={low:.0}-{high:.0} mg/dL, audio={audio_alert})."
            ),
        );
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn test_alert_now(mut self: Pin<&mut Self>) {
        let settings = current_settings_from_controller(&self);
        log_info("ALERT", "Triggered manual alert test from Settings.");
        qobject::librelinkup_trigger_window_alert(
            settings.bring_to_front_enabled,
            settings.flash_window_enabled,
        );
        if settings.audio_alert_enabled {
            let sound_path = if settings.use_custom_sound {
                QString::from(settings.custom_sound_path.as_str())
            } else {
                QString::from("")
            };
            qobject::librelinkup_play_alert_sound(&sound_path);
        }
        self.as_mut().sync_logs_to_properties();
    }

    pub fn load_demo_sample_data(mut self: Pin<&mut Self>) {
        let sample_points = vec![
            GraphPoint { timestamp: "08:00".to_string(), value_mg_dl: 98.0 },
            GraphPoint { timestamp: "08:30".to_string(), value_mg_dl: 104.0 },
            GraphPoint { timestamp: "09:00".to_string(), value_mg_dl: 118.0 },
            GraphPoint { timestamp: "09:30".to_string(), value_mg_dl: 132.0 },
            GraphPoint { timestamp: "10:00".to_string(), value_mg_dl: 125.0 },
            GraphPoint { timestamp: "10:30".to_string(), value_mg_dl: 114.0 },
            GraphPoint { timestamp: "11:00".to_string(), value_mg_dl: 109.0 },
            GraphPoint { timestamp: "11:30".to_string(), value_mg_dl: 112.0 },
        ];
        self.as_mut().rust_mut().snapshot = GlucoseSnapshot {
            value_mg_dl: 112.0,
            trend_arrow: 3,
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            target_low_mg_dl: 70.0,
            target_high_mg_dl: 180.0,
            patient_id: "demo-patient-001".to_string(),
            first_name: "Sample".to_string(),
            last_name: "Sensor".to_string(),
            graph_points: sample_points,
        };
        self.as_mut().set_logged_in(true);
        self.as_mut()
            .set_status_message(QString::from("Sample CGM telemetry loaded"));
        log_info("GLUCOSE", "Loaded sample CGM telemetry (112 mg/dL → Stable).");
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

    pub fn show_main_window(self: Pin<&mut Self>) {
        qobject::librelinkup_show_main_window();
    }

    pub fn hide_main_window(self: Pin<&mut Self>) {
        qobject::librelinkup_hide_main_window();
    }

    pub fn quit_application(self: Pin<&mut Self>) {
        qobject::librelinkup_quit_app();
    }
}
