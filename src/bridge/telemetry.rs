//! `TelemetryController` implementation: authentication, CGM polling, alerting, tray sync.
//!
//! This object is also the **service host**: it owns every background task (login, refresh,
//! update check/install) and it is the Qt object that the C++ tray/window managers hold a
//! pointer to, so the tray's actions are dispatched here and delegated into shared state.

use std::pin::Pin;

use cxx_qt::Threading;
use cxx_qt_lib::QString;

use crate::api::{authenticate_async, normalize_country_code, ApiError, AuthSuccess};
use crate::bridge::autostart::{
    background_runtime, fire_alert, is_flatpak, now_epoch, sync_autostart_desktop_file,
};
use crate::bridge::ops;
use crate::bridge::qobject_bridge::{qobject, TelemetryControllerRust};
use crate::bridge::state::{with_state, AppState};
use crate::crypto::{
    clear_default_credentials, load_default_credentials, save_default_credentials,
    try_load_host_ha_env, StoredCredentials,
};
use crate::event_log::{log_error, log_info, log_warn, mask_email};
use crate::models::{
    format_glucose_value, trend_arrow_symbol,
    DEFAULT_TARGET_HIGH_MG_DL, DEFAULT_TARGET_LOW_MG_DL,
};
use crate::session::{
    build_demo_snapshot, evaluate_alert_decision, fetch_snapshot_for_source_async,
    record_reauth_success, reset_reauth_backoff, RefreshOutcome,
};
use crate::updater::{check_for_updates_async, download_and_install_update_async, UpdateCheckResult};

/// Sets a `QString` property only when the value actually changed, so the QML binding graph
/// is not re-evaluated on every 250 ms tick.
macro_rules! sync_str {
    ($self:ident, $setter:ident, $getter:ident, $value:expr) => {{
        let new_value: String = $value;
        if $self.$getter().to_string() != new_value {
            $self.as_mut().$setter(QString::from(new_value.as_str()));
        }
    }};
}

/// Same for scalar properties.
macro_rules! sync_val {
    ($self:ident, $setter:ident, $getter:ident, $value:expr) => {{
        let new_value = $value;
        if *$self.$getter() != new_value {
            $self.as_mut().$setter(new_value);
        }
    }};
}

impl Default for TelemetryControllerRust {
    fn default() -> Self {
        // Valid empty values only: `initialize()` calls `sync()`, which fills every property
        // from the shared state. Building an `AppState` here would decrypt the history vault
        // once per controller instance.
        Self {
            logged_in: false,
            busy: false,
            status_message: QString::from("Ready"),
            error_message: QString::default(),
            username: QString::default(),
            patient_name: QString::default(),
            patient_id: QString::default(),
            glucose_display: QString::from("--"),
            glucose_mg_dl: 0.0,
            result_unit: QString::from("mg/dL"),
            trend_arrow: 3,
            trend_symbol: QString::from("→"),
            trend_description: QString::from("Stable"),
            range_status_label: QString::from("Waiting"),
            range_color_hex: QString::from("#22c55e"),
            target_range_display: QString::from("70–180 mg/dL"),
            target_low_mg_dl: DEFAULT_TARGET_LOW_MG_DL,
            target_high_mg_dl: DEFAULT_TARGET_HIGH_MG_DL,
            last_updated: QString::from("Never"),
            graph_points_json: QString::from("[]"),
            reading_age_display: QString::default(),
            reading_stale: false,
            alerts_snoozed: false,
            snooze_until_display: QString::default(),
            app_version: QString::from(env!("CARGO_PKG_VERSION")),
            credentials_encrypted: false,
            stats_window: QString::from("14d"),
            ea1c_headline: QString::from("eA1C --% • GMI --% • TIR --%"),
            ea1c_subline: QString::from("Accumulating CGM readings for A1C / GMI prediction..."),
            ea1c_display: QString::from("--%"),
            gmi_display: QString::from("--%"),
            tir_display: QString::from("--%"),
            accessible_summary: QString::from("Waiting for glucose reading."),
            sensor_status_display: QString::from("—"),
            sensor_serial: QString::default(),
            sensor_in_warmup: false,
            sensor_expiring_soon: false,
        }
    }
}

impl qobject::TelemetryController {
    /// Initializes this controller from shared state (QML calls this on `Component.onCompleted`).
    pub fn initialize(mut self: Pin<&mut Self>) {
        log_info(
            "SYSTEM",
            format!(
                "Initializing LibreLinkUp Desktop v{} (KDE Plasma 6 / Kirigami)",
                env!("CARGO_PKG_VERSION")
            ),
        );

        let (
            theme,
            ov_screen,
            ov_x,
            ov_y,
            ov_w,
            ov_h,
            ov_all,
            win_screen,
            win_x,
            win_y,
            win_w,
            win_h,
            autostart,
            minimized,
        ) = with_state(|s| {
            (
                s.theme_mode.clone(),
                s.overlay_screen_name.clone(),
                s.overlay_x,
                s.overlay_y,
                s.overlay_width,
                s.overlay_height,
                s.overlay_on_all_desktops,
                s.window_screen_name.clone(),
                s.window_x,
                s.window_y,
                s.window_width,
                s.window_height,
                s.autostart_enabled,
                s.start_minimized,
            )
        });

        let q_theme = QString::from(theme.as_str());
        qobject::librelinkup_apply_app_theme(&q_theme);
        qobject::librelinkup_restore_overlay_placement(
            &QString::from(ov_screen.as_str()),
            ov_x,
            ov_y,
            ov_w,
            ov_h,
        );
        qobject::librelinkup_restore_main_window_placement(
            &QString::from(win_screen.as_str()),
            win_x,
            win_y,
            win_w,
            win_h,
        );
        qobject::librelinkup_apply_overlay_all_desktops(ov_all);

        // Inside Flatpak, (re)register login autostart with the Background portal so the
        // entry exists and carries the current --minimized preference.
        if is_flatpak() && autostart {
            sync_autostart_desktop_file(true, minimized);
        }

        self.as_mut().restore_session();
        self.as_mut().sync();
        self.as_mut().sync_tray();

        if with_state(|s| s.check_updates_on_startup) && !is_offscreen() {
            self.as_mut().start_update_check(false);
        }
    }

    /// Restores a stored session (or reports why it could not be restored).
    fn restore_session(mut self: Pin<&mut Self>) {
        match load_default_credentials() {
            Ok(Some(mut creds)) if creds.has_valid_session() => {
                let mut normalized = false;
                if !creds.country.is_empty() {
                    let norm = normalize_country_code(&creds.country);
                    if norm != creds.country {
                        log_info(
                            "API",
                            format!(
                                "Normalized stored country code '{}' to regional endpoint '{}'.",
                                creds.country, norm
                            ),
                        );
                        creds.country = norm;
                        normalized = true;
                    }
                }
                if normalized {
                    let remember_pw = with_state(|s| s.remember_password);
                    let _ = save_default_credentials(&creds.sanitized_for_storage(remember_pw));
                }
                log_info(
                    "CRYPTO",
                    format!(
                        "Loaded AES-256-GCM encrypted credentials for region '{}' (account_id present).",
                        creds.country
                    ),
                );
                let username = creds.username.clone();
                let country = creds.country.clone();
                let language = creds.language.clone();
                let has_ha_tok = creds.has_valid_ha_session();
                let full_name = format!("{} {}", creds.first_name.trim(), creds.last_name.trim())
                    .trim()
                    .to_string();
                let should_refresh = with_state(|s| {
                    s.ha_token_configured = has_ha_tok;
                    s.credentials_encrypted = true;
                    if !username.is_empty() {
                        s.username = username;
                    } else if s.data_source == "home_assistant" && !s.ha_url.is_empty() {
                        s.username = s.ha_url.clone();
                    }
                    if !country.is_empty() {
                        s.country = country;
                    }
                    if !language.is_empty() {
                        s.language = language;
                    }
                    if !full_name.is_empty() {
                        s.patient_name = full_name;
                    }
                    let valid_active = creds.has_valid_session_for(&s.data_source, &s.ha_url);
                    s.logged_in = valid_active;
                    s.status_message = if valid_active {
                        if s.data_source == "home_assistant" {
                            "Connected to Home Assistant (AES-256-GCM Vault)".to_string()
                        } else {
                            "Authenticated (AES-256-GCM Vault)".to_string()
                        }
                    } else {
                        "Please sign in or configure Home Assistant".to_string()
                    };
                    s.creds = Some(creds);
                    valid_active
                });
                let _ = with_state(|s| ops::persist_settings(s));
                if should_refresh {
                    self.as_mut().refresh_glucose_now();
                }
            }
            Ok(_) => {
                log_info(
                    "AUTH",
                    "No stored session found in encrypted vault — awaiting login.",
                );
                with_state(|s| s.status_message = "Please sign in to LibreLinkUp".to_string());
            }
            Err(e) => {
                log_warn("CRYPTO", format!("Could not decrypt stored credentials: {e}"));
                with_state(|s| s.error_message = e);
            }
        }
    }

    /// Mirrors shared state into QML properties. Cheap: only pushes changed values.
    pub fn sync(mut self: Pin<&mut Self>) {
        let snapshot = with_state(|s| {
            (
                s.logged_in,
                s.busy,
                s.status_message.clone(),
                s.error_message.clone(),
                s.username.clone(),
                s.patient_name.clone(),
                s.patient_id.clone(),
                s.glucose_display.clone(),
                s.glucose_mg_dl,
                s.result_unit.clone(),
                s.trend_arrow,
                s.trend_symbol.clone(),
                s.trend_description.clone(),
                s.range_status_label.clone(),
                s.range_color_hex.clone(),
                s.target_range_display.clone(),
                s.target_low_mg_dl,
                s.target_high_mg_dl,
                s.last_updated.clone(),
                s.graph_points_json.clone(),
                s.reading_age_display.clone(),
                s.reading_stale,
                s.alerts_snoozed,
                s.snooze_until_display.clone(),
                s.app_version.clone(),
                s.credentials_encrypted,
                s.stats_window.clone(),
                s.ea1c_headline.clone(),
                s.ea1c_subline.clone(),
                s.ea1c_display.clone(),
                s.gmi_display.clone(),
                s.tir_display.clone(),
            )
        });
        let (
            logged_in,
            busy,
            status_message,
            error_message,
            username,
            patient_name,
            patient_id,
            glucose_display,
            glucose_mg_dl,
            result_unit,
            trend_arrow,
            trend_symbol,
            trend_description,
            range_status_label,
            range_color_hex,
            target_range_display,
            target_low,
            target_high,
            last_updated,
            graph_points_json,
            reading_age_display,
            reading_stale,
            alerts_snoozed,
            snooze_until_display,
            app_version,
            credentials_encrypted,
            stats_window,
            ea1c_headline,
            ea1c_subline,
            ea1c_display,
            gmi_display,
            tir_display,
        ) = snapshot;

        sync_val!(self, set_logged_in, logged_in, logged_in);
        sync_val!(self, set_busy, busy, busy);
        sync_str!(self, set_status_message, status_message, status_message);
        sync_str!(self, set_error_message, error_message, error_message);
        sync_str!(self, set_username, username, username);
        sync_str!(self, set_patient_name, patient_name, patient_name);
        sync_str!(self, set_patient_id, patient_id, patient_id);
        sync_str!(self, set_glucose_display, glucose_display, glucose_display);
        sync_val!(self, set_glucose_mg_dl, glucose_mg_dl, glucose_mg_dl);
        sync_str!(self, set_result_unit, result_unit, result_unit);
        sync_val!(self, set_trend_arrow, trend_arrow, trend_arrow);
        sync_str!(self, set_trend_symbol, trend_symbol, trend_symbol);
        sync_str!(
            self,
            set_trend_description,
            trend_description,
            trend_description
        );
        sync_str!(self, set_range_status_label, range_status_label, range_status_label);
        sync_str!(self, set_range_color_hex, range_color_hex, range_color_hex);
        sync_str!(
            self,
            set_target_range_display,
            target_range_display,
            target_range_display
        );
        sync_val!(self, set_target_low_mg_dl, target_low_mg_dl, target_low);
        sync_val!(self, set_target_high_mg_dl, target_high_mg_dl, target_high);
        sync_str!(self, set_last_updated, last_updated, last_updated);
        sync_str!(self, set_graph_points_json, graph_points_json, graph_points_json);
        sync_str!(
            self,
            set_reading_age_display,
            reading_age_display,
            reading_age_display
        );
        sync_val!(self, set_reading_stale, reading_stale, reading_stale);
        sync_val!(self, set_alerts_snoozed, alerts_snoozed, alerts_snoozed);
        sync_str!(
            self,
            set_snooze_until_display,
            snooze_until_display,
            snooze_until_display
        );
        sync_str!(self, set_app_version, app_version, app_version);
        sync_val!(
            self,
            set_credentials_encrypted,
            credentials_encrypted,
            credentials_encrypted
        );
        sync_str!(self, set_stats_window, stats_window, stats_window);
        sync_str!(self, set_ea1c_headline, ea1c_headline, ea1c_headline);
        sync_str!(self, set_ea1c_subline, ea1c_subline, ea1c_subline);
        sync_str!(self, set_ea1c_display, ea1c_display, ea1c_display);
        sync_str!(self, set_gmi_display, gmi_display, gmi_display);
        sync_str!(self, set_tir_display, tir_display, tir_display);
        let (acc, sensor_status, sensor_sn, warmup, exp_soon) = with_state(|s| {
            (
                s.accessible_summary.clone(),
                s.sensor_status_display.clone(),
                s.sensor_serial.clone(),
                s.sensor_in_warmup,
                s.sensor_expiring_soon,
            )
        });
        sync_str!(self, set_accessible_summary, accessible_summary, acc);
        sync_str!(
            self,
            set_sensor_status_display,
            sensor_status_display,
            sensor_status
        );
        sync_str!(self, set_sensor_serial, sensor_serial, sensor_sn);
        sync_val!(self, set_sensor_in_warmup, sensor_in_warmup, warmup);
        sync_val!(self, set_sensor_expiring_soon, sensor_expiring_soon, exp_soon);
    }

    /// The single periodic entry point: apply derived state, refresh the tray, evaluate alerts.
    pub fn tick(mut self: Pin<&mut Self>) {
        with_state(|s| {
            ops::refresh_display(s);
            ops::refresh_analytics(s);
            ops::refresh_medication_views(s);
            ops::refresh_logs(s);
            s.busy = s.login_in_flight || s.active_refresh.is_some();
        });
        self.as_mut().sync();
        self.as_mut().sync_tray();
        self.as_mut().evaluate_alerts();
    }

    pub fn evaluate_staleness(mut self: Pin<&mut Self>) {
        self.as_mut().tick();
    }

    /// Starts an asynchronous LibreLinkUp login. Returns `true` when the request was started;
    /// the outcome is reported through the properties once the background task completes.
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

        if with_state(|s| s.login_in_flight) {
            return false;
        }
        if user_str.len() < 2 || pass_str.len() < 2 {
            let msg = "Please enter both your LibreLinkUp email and password.";
            with_state(|s| {
                s.error_message = msg.to_string();
                s.status_message = "Authentication failed".to_string();
            });
            log_warn("AUTH", msg);
            self.as_mut().sync();
            return false;
        }

        let Some(runtime) = background_runtime() else {
            with_state(|s| s.error_message = "Failed to initialize async runtime.".to_string());
            log_error("SYSTEM", "Failed to initialize async runtime.");
            self.as_mut().sync();
            return false;
        };

        // A new session supersedes any refresh still running for the previous one.
        let generation = with_state(|s| {
            s.session_generation = s.session_generation.wrapping_add(1);
            s.active_refresh = None;
            s.login_in_flight = true;
            s.busy = true;
            s.error_message.clear();
            s.status_message = "Signing in to LibreLinkUp...".to_string();
            reset_reauth_backoff();
            s.session_generation
        });
        log_info(
            "AUTH",
            format!(
                "Attempting LibreLinkUp login for '{}' (region: {country_str})",
                mask_email(&user_str)
            ),
        );
        self.as_mut().sync();

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
        let current = with_state(|s| s.session_generation);
        if generation != current {
            with_state(|s| {
                s.login_in_flight = false;
                s.busy = false;
            });
            self.as_mut().sync();
            return;
        }
        with_state(|s| {
            s.login_in_flight = false;
            s.busy = false;
        });

        match auth_res {
            Ok(auth) => {
                let existing_ha_token = with_state(|s| {
                    s.creds
                        .as_ref()
                        .map(|c| c.ha_token.clone())
                        .unwrap_or_default()
                });
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
                    ha_token: existing_ha_token,
                };
                let remember_pw = with_state(|s| s.remember_password);
                match save_default_credentials(&stored.sanitized_for_storage(remember_pw)) {
                    Ok(()) => {
                        log_info(
                            "CRYPTO",
                            "Encrypted and saved session credentials with AES-256-GCM + PBKDF2-HMAC-SHA256.",
                        );
                        with_state(|s| s.credentials_encrypted = true);
                    }
                    Err(e) => log_error("CRYPTO", format!("Failed to save encrypted credentials: {e}")),
                }

                with_state(|s| {
                    s.data_source = "librelinkup".to_string();
                    s.username = user_str;
                    s.country = auth.country.clone();
                    s.language = lang_str;
                    s.logged_in = true;
                    s.creds = Some(stored);
                    s.status_message = "Signed in — fetching glucose data...".to_string();
                });
                let _ = with_state(|s| ops::persist_settings(s));
                log_info(
                    "AUTH",
                    format!("Login succeeded (resolved region: '{}').", auth.country),
                );
                self.as_mut().sync();
                self.as_mut().refresh_glucose_now();
            }
            Err(err) => {
                let msg = err.to_string();
                log_error("AUTH", format!("Login failed: {msg}"));
                with_state(|s| {
                    s.error_message = msg;
                    s.status_message = "Authentication failed".to_string();
                });
                self.as_mut().sync();
            }
        }
    }

    pub fn logout(mut self: Pin<&mut Self>) {
        let _ = clear_default_credentials();
        // Persist any pending history before wiping the session.
        with_state(|s| ops::persist_history_if_due(s, true));
        reset_reauth_backoff();
        with_state(|s| {
            s.session_generation = s.session_generation.wrapping_add(1);
            s.login_in_flight = false;
            s.active_refresh = None;
            s.creds = None;
            s.snapshot = crate::models::GlucoseSnapshot::default();
            s.last_alert = None;
            s.last_successful_fetch_epoch = None;
            s.logged_in = false;
            s.credentials_encrypted = false;
            s.ha_token_configured = false;
            s.patient_name.clear();
            s.patient_id.clear();
            s.error_message.clear();
            s.status_message = "Signed out — encrypted vault cleared.".to_string();
        });
        log_info(
            "AUTH",
            "User signed out and encrypted credentials file deleted.",
        );
        with_state(ops::refresh_display);
        self.as_mut().sync();
        self.as_mut().sync_tray();
    }

    /// Saves Home Assistant LibreLink integration settings and Long-Lived Access Token into the
    /// AES-256-GCM encrypted vault, switches data source if requested, and triggers an immediate poll.
    pub fn save_home_assistant_config(
        mut self: Pin<&mut Self>,
        data_source: QString,
        ha_url: QString,
        ha_token: QString,
        ha_sensor_entity_id: QString,
    ) -> bool {
        let src_clean = if data_source
            .to_string()
            .trim()
            .eq_ignore_ascii_case("home_assistant")
        {
            "home_assistant".to_string()
        } else {
            "librelinkup".to_string()
        };
        let raw_url = ha_url.to_string().trim().to_string();
        let clean_url = if raw_url.is_empty() {
            String::new()
        } else {
            match crate::api::validate_ha_base_url(&raw_url) {
                Ok(u) => u,
                Err(e) => {
                    let msg = e.to_string();
                    log_warn("API", format!("Invalid Home Assistant URL: {msg}"));
                    with_state(|s| s.error_message = msg);
                    self.as_mut().sync();
                    return false;
                }
            }
        };
        let clean_entity = ha_sensor_entity_id.to_string().trim().to_string();
        let new_tok = ha_token.to_string().trim().to_string();

        let (mut creds, remember_pw) = with_state(|s| {
            (
                s.creds.clone().unwrap_or_else(|| {
                    let mut c = StoredCredentials::default();
                    c.country = s.country.clone();
                    c.language = s.language.clone();
                    c
                }),
                s.remember_password,
            )
        });
        if !new_tok.is_empty() {
            creds.ha_token = new_tok;
        }

        if src_clean == "home_assistant" && (clean_url.is_empty() || !creds.has_valid_ha_session())
        {
            let msg = "Please enter both your Home Assistant URL and Long-Lived Access Token.";
            with_state(|s| s.error_message = msg.to_string());
            log_warn("API", msg);
            self.as_mut().sync();
            return false;
        }

        if creds.has_valid_session() {
            if let Err(e) = save_default_credentials(&creds.sanitized_for_storage(remember_pw)) {
                log_error("CRYPTO", format!("Failed to save encrypted credentials: {e}"));
            }
        }

        let valid_active = creds.has_valid_session_for(&src_clean, &clean_url);
        let has_ha_tok = creds.has_valid_ha_session();
        with_state(|s| {
            s.session_generation = s.session_generation.wrapping_add(1);
            s.active_refresh = None;
            s.data_source = src_clean.clone();
            s.ha_url = clean_url.clone();
            s.ha_sensor_entity_id = clean_entity.clone();
            s.ha_token_configured = has_ha_tok;
            s.credentials_encrypted = creds.has_valid_session();
            s.creds = Some(creds);
            s.logged_in = valid_active;
            s.error_message.clear();
            if src_clean == "home_assistant" && valid_active {
                s.username = clean_url.clone();
                s.status_message = "Connecting to Home Assistant LibreLink...".to_string();
            } else if valid_active {
                s.status_message = "Authenticated (LibreLinkUp Cloud)".to_string();
            }
        });
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "API",
            format!(
                "Saved data source configuration: source='{src_clean}', ha_url='{clean_url}', entity='{}'",
                if clean_entity.is_empty() {
                    "auto-discover"
                } else {
                    &clean_entity
                }
            ),
        );
        self.as_mut().sync();
        if valid_active {
            self.as_mut().refresh_glucose_now();
        }
        true
    }

    /// Reads `HA_URL` and `HA_TOKEN` from `~/.config/home-assistant.env` on the local machine
    /// and connects to the Home Assistant LibreLink integration immediately.
    pub fn auto_fill_home_assistant_from_env(mut self: Pin<&mut Self>) -> bool {
        let Some((url, token)) = try_load_host_ha_env() else {
            let msg = "Could not find HA_URL and HA_TOKEN in ~/.config/home-assistant.env";
            log_warn("API", msg);
            with_state(|s| s.error_message = msg.to_string());
            self.as_mut().sync();
            return false;
        };
        let entity = with_state(|s| s.ha_sensor_entity_id.clone());
        log_info(
            "API",
            format!("Loaded Home Assistant URL '{url}' and token from ~/.config/home-assistant.env"),
        );
        self.as_mut().save_home_assistant_config(
            QString::from("home_assistant"),
            QString::from(url.as_str()),
            QString::from(token.as_str()),
            QString::from(entity.as_str()),
        )
    }

    /// Selects a specific Home Assistant patient glucose sensor entity (or empty string for
    /// auto-discover), persists the setting immediately, and triggers a fresh CGM poll if
    /// connected to Home Assistant.
    pub fn select_ha_patient(mut self: Pin<&mut Self>, entity_id: QString) {
        let clean_entity = entity_id.to_string().trim().to_string();
        let should_refresh = with_state(|s| {
            s.ha_sensor_entity_id = clean_entity.clone();
            s.active_refresh = None;
            let _ = ops::persist_settings(s);
            s.data_source == "home_assistant"
                && s.logged_in
                && s.creds
                    .as_ref()
                    .is_some_and(|c| c.has_valid_session_for(&s.data_source, &s.ha_url))
        });
        log_info(
            "API",
            format!(
                "Selected Home Assistant patient sensor entity: '{}'",
                if clean_entity.is_empty() {
                    "auto-discover"
                } else {
                    &clean_entity
                }
            ),
        );
        self.as_mut().sync();
        if should_refresh {
            self.as_mut().refresh_glucose_now();
        }
    }

    /// Starts an asynchronous CGM refresh (no-op if one is already running).
    pub fn refresh_glucose_now(mut self: Pin<&mut Self>) {
        if with_state(|s| s.active_refresh.is_some() || s.login_in_flight) {
            return;
        }
        let (Some(creds), data_source, ha_url, ha_entity) = with_state(|s| {
            (
                s.creds.clone(),
                s.data_source.clone(),
                s.ha_url.clone(),
                s.ha_sensor_entity_id.clone(),
            )
        }) else {
            return;
        };
        if !creds.has_valid_session_for(&data_source, &ha_url) {
            return;
        }
        let Some(runtime) = background_runtime() else {
            with_state(|s| s.error_message = "Failed to initialize async runtime.".to_string());
            return;
        };

        let (generation, seq) = with_state(|s| {
            s.refresh_seq = s.refresh_seq.wrapping_add(1);
            s.active_refresh = Some(s.refresh_seq);
            s.busy = true;
            s.error_message.clear();
            (s.session_generation, s.refresh_seq)
        });
        self.as_mut().sync();

        let qt_thread = self.qt_thread();
        runtime.spawn(async move {
            let outcome =
                fetch_snapshot_for_source_async(&data_source, &ha_url, &ha_entity, &creds).await;
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

    pub fn next_poll_interval_ms(self: Pin<&mut Self>) -> i32 {
        let base = with_state(|s| s.poll_interval_seconds.max(1) as u32);
        (crate::models::jittered_poll_interval(base) as i32) * 1000
    }

    /// Tray/C++ entry point (the tray menu calls `refreshGlucoseNow`).
    pub fn refresh_glucose_from_tray(self: Pin<&mut Self>) {
        self.refresh_glucose_now();
    }

    fn apply_refresh_outcome(
        mut self: Pin<&mut Self>,
        generation: u64,
        seq: u64,
        outcome: Result<RefreshOutcome, ApiError>,
    ) {
        let still_current = with_state(|s| {
            if s.active_refresh == Some(seq) {
                s.active_refresh = None;
            }
            s.busy = s.login_in_flight || s.active_refresh.is_some();
            generation == s.session_generation
        });
        if !still_current {
            self.as_mut().sync();
            return;
        }

        match outcome {
            Ok(outcome) => {
                if let Some(renewed) = outcome.updated_credentials {
                    record_reauth_success();
                    let remember_pw = with_state(|s| s.remember_password);
                    if let Err(e) =
                        save_default_credentials(&renewed.sanitized_for_storage(remember_pw))
                    {
                        log_error("CRYPTO", format!("Failed to save renewed credentials: {e}"));
                    }
                    with_state(|s| s.creds = Some(renewed));
                }

                let snap = outcome.snapshot;
                let ha_states = outcome.ha_states;
                let unit = with_state(|s| s.result_unit.clone());
                log_info(
                    "GLUCOSE",
                    format!(
                        "Fetched glucose reading: {} {} {} ({} points)",
                        format_glucose_value(snap.value_mg_dl, &unit),
                        unit,
                        trend_arrow_symbol(snap.trend_arrow),
                        snap.graph_points.len()
                    ),
                );

                let mut push_initial_ha_meds = false;
                with_state(|s| {
                    s.last_successful_fetch_epoch = Some(now_epoch());
                    ops::ingest_history(s, &snap);
                    s.snapshot = snap;
                    s.status_message = "Live CGM telemetry active".to_string();
                    if let Some(ref states_arr) = ha_states {
                        let discovered = crate::api::discover_ha_patients(states_arr);
                        s.ha_discovered_patients_json =
                            serde_json::to_string(&discovered).unwrap_or_else(|_| "[]".to_string());
                        if s.ha_sync_medications {
                            let has_ha_meds_entity = states_arr.iter().any(|e| {
                                e.get("entity_id")
                                    .and_then(|v| v.as_str())
                                    .is_some_and(|id| id == "sensor.librelinkup_medications")
                            });
                            if s.medication_store.merge_from_ha_states(states_arr) {
                                let _ = crate::medications::save_default_medications(
                                    &s.medication_store,
                                );
                                ops::refresh_medication_views(s);
                                log_info(
                                    "MEDS",
                                    "Synchronized remote medications/doses from Home Assistant.",
                                );
                            } else if !has_ha_meds_entity && !s.medication_store.medications.is_empty() {
                                push_initial_ha_meds = true;
                            }
                        }
                        if s.ha_sync_snooze {
                            if let Some((remote_until, remote_updated)) =
                                crate::api::parse_ha_remote_snooze(
                                    states_arr,
                                    s.snooze_updated_at_epoch,
                                )
                            {
                                if s.snooze_until_epoch != remote_until {
                                    s.snooze_until_epoch = remote_until;
                                    s.snooze_updated_at_epoch = remote_updated;
                                    if remote_until == 0 {
                                        s.last_alert = None;
                                    }
                                    let _ = ops::persist_settings(s);
                                    log_info(
                                        "ALERT",
                                        "Synchronized cross-device alert snooze state from Home Assistant.",
                                    );
                                }
                            }
                        }
                    }
                    ops::refresh_display(s);
                });
                if push_initial_ha_meds {
                    ops::spawn_ha_medications_push(None);
                }
            }
            Err(err) => {
                let msg = err.to_string();
                // Re-authentication failures record their own backoff in `session`; a plain
                // 401 just surfaces the error and waits for the next poll.
                log_warn("API", format!("Failed to fetch CGM data: {msg}"));
                with_state(|s| s.error_message = msg);
            }
        }

        // Evaluate after both success and failure: a failing fetch can still mean the
        // last reading has gone stale.
        self.as_mut().tick();
    }

    pub fn load_demo_sample_data(mut self: Pin<&mut Self>) {
        let now = now_epoch();
        let snap = build_demo_snapshot(now);
        let demo_ha_patients = vec![
            crate::api::HaPatientOption {
                entity_id: "sensor.alex_patient_measurement".to_string(),
                patient_name: "Alex Patient".to_string(),
                reading_display: "112 mg/dL".to_string(),
                label: "Alex Patient (112 mg/dL) — sensor.alex_patient_measurement".to_string(),
            },
            crate::api::HaPatientOption {
                entity_id: "sensor.sam_patient_measurement".to_string(),
                patient_name: "Sam Patient".to_string(),
                reading_display: "138 mg/dL".to_string(),
                label: "Sam Patient (138 mg/dL) — sensor.sam_patient_measurement".to_string(),
            },
        ];
        with_state(|s| {
            s.glucose_history.ingest_snapshot(&snap, now);
            s.medication_store = crate::medications::build_demo_medication_store(now);
            s.ha_discovered_patients_json =
                serde_json::to_string(&demo_ha_patients).unwrap_or_else(|_| "[]".to_string());
            s.snapshot = snap;
            s.last_successful_fetch_epoch = Some(now);
            s.logged_in = true;
            s.status_message = "Sample CGM telemetry loaded".to_string();
            ops::refresh_display(s);
        });
        log_info(
            "GLUCOSE",
            "Loaded sample CGM telemetry (112 mg/dL → Stable).",
        );
        self.as_mut().tick();
    }

    pub fn update_stats_window(mut self: Pin<&mut Self>, window_code: QString) {
        let code = crate::analytics::StatsWindow::from_code(&window_code.to_string())
            .code()
            .to_string();
        with_state(|s| s.stats_window = code);
        let _ = with_state(|s| ops::persist_settings(s));
        self.as_mut().tick();
    }

    pub fn snooze_alerts(mut self: Pin<&mut Self>, minutes: i32) {
        let now = now_epoch();
        let until = if minutes > 0 {
            now + i64::from(minutes.min(24 * 60)) * 60
        } else {
            0
        };
        with_state(|s| {
            s.snooze_until_epoch = until;
            s.snooze_updated_at_epoch = now;
            if until == 0 {
                // Re-arm so a still-active condition alerts on the next evaluation.
                s.last_alert = None;
            }
        });
        if until > 0 {
            log_info(
                "ALERT",
                format!("Alerts snoozed for {minutes} min (urgent low still alerts)."),
            );
        } else {
            log_info("ALERT", "Alert snooze cancelled.");
        }
        let _ = with_state(|s| ops::persist_settings(s));
        ops::spawn_ha_snooze_push();
        self.as_mut().tick();
    }

    pub fn handle_notification_action(mut self: Pin<&mut Self>, action_key: QString) -> bool {
        let key = action_key.to_string();
        if let Some(med_id) = key.trim().strip_prefix("med_taken:") {
            log_info(
                "MEDS",
                format!("Desktop reminder action 'Taken' clicked for medication '{med_id}'."),
            );
            let logged = with_state(|s| {
                let now = now_epoch();
                let g = (s.snapshot.value_mg_dl > 0.0).then_some(s.snapshot.value_mg_dl);
                let res = s.medication_store.log_dose(
                    med_id.trim(),
                    None,
                    now,
                    g,
                    "Logged from desktop reminder",
                );
                if res.is_ok() {
                    let _ = crate::medications::save_default_medications(&s.medication_store);
                }
                res.ok()
            });
            if let Some(ref dose) = logged {
                ops::spawn_ha_medications_push(Some(dose.clone()));
            }
            self.as_mut().tick();
            return logged.is_some();
        }
        if let Some(med_id) = key.trim().strip_prefix("med_skip:") {
            let skipped = with_state(|s| {
                let res = s.medication_store.skip_dose(
                    med_id.trim(),
                    now_epoch(),
                    "Skipped from desktop reminder",
                );
                if res.is_ok() {
                    let _ = crate::medications::save_default_medications(&s.medication_store);
                }
                res.ok()
            });
            if let Some(ref dose) = skipped {
                ops::spawn_ha_medications_push(Some(dose.clone()));
            }
            self.as_mut().tick();
            return skipped.is_some();
        }
        if let Some(minutes) = crate::models::parse_notification_snooze_action(&key) {
            log_info(
                "ALERT",
                format!("Desktop notification action '{key}' clicked -> snoozing alerts for {minutes} min."),
            );
            self.snooze_alerts(minutes);
            return true;
        }
        if key.trim() == "default" {
            self.show_main_window();
            return true;
        }
        false
    }

    // ---- settings-shaped actions dispatched by the C++ tray / window manager ----

    pub fn update_theme_mode(mut self: Pin<&mut Self>, mode: QString) {
        let clean = mode.to_string().trim().to_lowercase();
        let valid = match clean.as_str() {
            "dark" | "light" | "system" => clean,
            _ => "system".to_string(),
        };
        qobject::librelinkup_apply_app_theme(&QString::from(valid.as_str()));
        with_state(|s| s.theme_mode = valid.clone());
        let _ = with_state(|s| ops::persist_settings(s));
        log_info("SYSTEM", format!("Switched color theme mode to '{valid}'."));
        self.as_mut().tick();
    }

    pub fn update_show_glucose_in_tray_icon(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.show_glucose_in_tray_icon = enabled);
        let _ = with_state(|s| ops::persist_settings(s));
        self.as_mut().tick();
    }

    pub fn update_show_glucose_in_tray_tooltip(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.show_glucose_in_tray_tooltip = enabled);
        let _ = with_state(|s| ops::persist_settings(s));
        self.as_mut().tick();
    }

    pub fn update_overlay_visible(mut self: Pin<&mut Self>, visible: bool) {
        with_state(|s| {
            s.overlay_visible = visible;
            if visible && s.window_mode == "windowed" {
                s.window_mode = "overlay".to_string();
            } else if !visible {
                s.window_mode = "windowed".to_string();
                s.overlay_only_mode = false;
            }
        });
        if visible {
            let (screen, x, y, w, h) = with_state(|s| {
                (
                    s.overlay_screen_name.clone(),
                    s.overlay_x,
                    s.overlay_y,
                    s.overlay_width,
                    s.overlay_height,
                )
            });
            qobject::librelinkup_restore_overlay_placement(&QString::from(screen.as_str()), x, y, w, h);
        }
        let _ = with_state(|s| ops::persist_settings(s));
        self.as_mut().tick();
    }

    pub fn update_overlay_on_all_desktops(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.overlay_on_all_desktops = enabled);
        qobject::librelinkup_apply_overlay_all_desktops(enabled);
        let _ = with_state(|s| ops::persist_settings(s));
        self.as_mut().tick();
    }

    pub fn update_overlay_only_mode(mut self: Pin<&mut Self>, only_overlay: bool) {
        with_state(|s| {
            let effective = only_overlay && s.overlay_visible;
            if s.overlay_only_mode != effective {
                s.overlay_only_mode = effective;
                let _ = ops::persist_settings(s);
            }
        });
        self.as_mut().tick();
    }

    pub fn update_autostart_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        let minimized = with_state(|s| {
            s.autostart_enabled = enabled;
            s.start_minimized
        });
        sync_autostart_desktop_file(enabled, minimized);
        let _ = with_state(|s| ops::persist_settings(s));
        self.as_mut().tick();
    }

    pub fn show_main_window(mut self: Pin<&mut Self>) {
        with_state(|s| {
            s.overlay_only_mode = false;
            let _ = ops::persist_settings(s);
        });
        qobject::librelinkup_show_main_window();
        let stale_check = !is_offscreen() && with_state(|s| {
            s.check_updates_on_startup && now_epoch() - s.last_update_check_epoch >= 3600
        });
        if stale_check {
            self.as_mut().start_update_check(false);
        }
        self.as_mut().tick();
    }

    pub fn hide_main_window(mut self: Pin<&mut Self>) {
        with_state(|s| {
            s.overlay_only_mode = s.overlay_visible;
            let _ = ops::persist_settings(s);
        });
        qobject::librelinkup_hide_main_window();
        self.as_mut().tick();
    }

    pub fn quit_application(self: Pin<&mut Self>) {
        // Flush any pending glucose history before the process goes away.
        with_state(|s| ops::persist_history_if_due(s, true));
        qobject::librelinkup_quit_app();
    }

    pub fn popup_tray_menu_at_cursor(self: Pin<&mut Self>) {
        qobject::librelinkup_popup_tray_menu();
    }

    // ---- update check / install (the only async owner besides login/refresh) ----

    pub fn check_for_updates(mut self: Pin<&mut Self>) {
        self.as_mut().start_update_check(true);
    }

    pub fn check_for_updates_silent(mut self: Pin<&mut Self>) {
        if with_state(|s| s.check_updates_on_startup) && !is_offscreen() {
            self.as_mut().start_update_check(false);
        }
    }

    pub fn install_update(mut self: Pin<&mut Self>) {
        if with_state(|s| s.installing_update) {
            return;
        }
        let (download_url, latest_ver) = with_state(|s| {
            (s.update_download_url.clone(), s.latest_version.clone())
        });
        let Some(runtime) = background_runtime() else {
            return;
        };
        with_state(|s| {
            s.installing_update = true;
            s.update_status_message = format!("Downloading and installing {latest_ver}...");
        });
        log_info(
            "SYSTEM",
            format!("Downloading and installing update {latest_ver} from {download_url}..."),
        );
        self.as_mut().tick();

        let qt_thread = self.qt_thread();
        runtime.spawn(async move {
            let outcome = download_and_install_update_async(&download_url, &latest_ver).await;
            let _ = qt_thread.queue(move |qobject| {
                qobject.apply_install_update_outcome(outcome);
            });
        });
    }

    fn apply_install_update_outcome(
        mut self: Pin<&mut Self>,
        outcome: Result<std::path::PathBuf, String>,
    ) {
        with_state(|s| s.installing_update = false);
        match outcome {
            Ok(path) => {
                let msg = format!(
                    "Update downloaded ({}) — installing and restarting...",
                    path.display()
                );
                log_info("SYSTEM", msg.clone());
                with_state(|s| s.update_status_message = msg);
            }
            Err(err) => {
                with_state(|s| s.update_status_message = err.clone());
                log_error("SYSTEM", format!("Auto-update install failed: {err}"));
            }
        }
        self.as_mut().tick();
    }

    pub(crate) fn start_update_check(mut self: Pin<&mut Self>, manual: bool) {
        if with_state(|s| s.checking_for_updates) {
            return;
        }
        let Some(runtime) = background_runtime() else {
            return;
        };
        with_state(|s| {
            s.checking_for_updates = true;
            s.update_status_message = "Checking GitHub Releases for updates...".to_string();
        });
        if manual {
            log_info(
                "SYSTEM",
                "Checking GitHub Releases for application updates...",
            );
        }
        self.as_mut().tick();

        let current_version = env!("CARGO_PKG_VERSION").to_string();
        let qt_thread = self.qt_thread();
        runtime.spawn(async move {
            let outcome = check_for_updates_async(&current_version).await;
            let _ = qt_thread.queue(move |qobject| {
                qobject.apply_update_check_outcome(manual, outcome);
            });
        });
    }

    fn apply_update_check_outcome(
        mut self: Pin<&mut Self>,
        manual: bool,
        outcome: Result<UpdateCheckResult, String>,
    ) {
        with_state(|s| {
            s.checking_for_updates = false;
            s.last_update_check_epoch = now_epoch();
        });
        match outcome {
            Ok(res) => {
                let already_notified =
                    with_state(|s| s.last_notified_update_version == res.latest_version);
                with_state(|s| {
                    s.update_available = res.update_available;
                    s.latest_version = res.latest_version.clone();
                    s.update_url = res.release_url.clone();
                    s.update_download_url = res.download_url.clone();
                    s.update_status_message = res.message.clone();
                });
                if res.update_available {
                    log_info(
                        "SYSTEM",
                        format!(
                            "Update available: {} (current {}). Release page: {}",
                            res.latest_version, res.current_version, res.release_url
                        ),
                    );
                    if manual || !already_notified {
                        with_state(|s| {
                            s.last_notified_update_version = res.latest_version.clone()
                        });
                        qobject::librelinkup_show_notification(
                            &QString::from("LibreLinkUp Update Available"),
                            &QString::from(
                                format!(
                                    "Version {} is available on GitHub Releases (you have {}).",
                                    res.latest_version, res.current_version
                                )
                                .as_str(),
                            ),
                            false,
                            false,
                        );
                    }
                } else if manual {
                    log_info("SYSTEM", format!("Update check complete: {}", res.message));
                    qobject::librelinkup_show_notification(
                        &QString::from("LibreLinkUp Up to Date"),
                        &QString::from(res.message.as_str()),
                        false,
                        false,
                    );
                }
            }
            Err(err) => {
                with_state(|s| s.update_status_message = err.clone());
                if manual {
                    log_warn("SYSTEM", format!("Update check failed: {err}"));
                }
            }
        }
        self.as_mut().tick();
    }

    // ---- tray presentation ----

    /// Pushes the tray icon/tooltip/menu state into C++.
    pub(crate) fn sync_tray(mut self: Pin<&mut Self>) {
        let payload = with_state(build_tray_payload);
        qobject::librelinkup_sync_tray(self.as_mut(), payload);
    }

    /// Fires alerts (glucose thresholds and medication reminders) when warranted.
    pub(crate) fn evaluate_alerts(self: Pin<&mut Self>) {
        let now = now_epoch();

        // Medication reminders run even when not polling glucose.
        let due = with_state(|s| {
            s.medication_store
                .evaluate_due_reminders(now, &mut s.medication_last_notified)
        });
        for reminder in due {
            log_info("MEDS", format!("{}: {}", reminder.title, reminder.body));
            qobject::librelinkup_show_medication_notification(
                &QString::from(reminder.medication_id.as_str()),
                &QString::from(reminder.title.as_str()),
                &QString::from(reminder.body.as_str()),
            );
        }

        let (settings, snapshot, last, logged_in, last_fetch) = with_state(|s| {
            (
                ops::current_settings(s),
                s.snapshot.clone(),
                s.last_alert,
                s.logged_in,
                s.last_successful_fetch_epoch,
            )
        });
        if !logged_in {
            return;
        }

        let (decision, cleared_previous) =
            evaluate_alert_decision(&settings, &snapshot, last, now, last_fetch);
        if cleared_previous {
            log_info("ALERT", "Glucose back within alert thresholds.");
            with_state(|s| s.last_alert = None);
        }

        if let Some(alert) = decision {
            log_warn("ALERT", format!("{}: {}", alert.title, alert.body));
            fire_alert(&settings, alert.title, &alert.body, alert.critical);
            with_state(|s| s.last_alert = Some((alert.kind, now)));
        }

        // Sensor Expiry Countdown Notification (once every 6 hours when within threshold)
        let should_notify_sensor_expiry = with_state(|s| {
            s.sensor_expiring_soon
                && !s.alerts_snoozed
                && (now - s.last_sensor_expiry_alert_epoch >= 6 * 3600)
        });
        if should_notify_sensor_expiry && !is_offscreen() {
            let status_txt = snapshot.format_sensor_status(now);
            with_state(|s| s.last_sensor_expiry_alert_epoch = now);
            log_warn("ALERT", format!("CGM Sensor Expiring Soon: {status_txt}"));
            qobject::librelinkup_show_notification(
                &QString::from("CGM Sensor Expiring Soon"),
                &QString::from(
                    format!("Active sensor countdown: {status_txt}. Prepare a replacement sensor.")
                        .as_str(),
                ),
                false,
                true,
            );
        }
    }
}

/// Whether Qt is running headless (the offscreen UI-test harness).
fn is_offscreen() -> bool {
    std::env::var_os("QT_QPA_PLATFORM")
        .map(|v| v == std::ffi::OsStr::new("offscreen"))
        .unwrap_or(false)
}

/// Builds the tray payload from shared state.
fn build_tray_payload(s: &mut AppState) -> qobject::TrayPayload {
    let settings = ops::current_settings(s);
    let snapshot = s.snapshot.clone();

    let badge_text = settings.format_tray_icon_badge(&snapshot);
    let mut tooltip_text = settings.format_tray_hover_tooltip(&snapshot);
    if settings.show_glucose_in_tray_tooltip && !s.ea1c_headline.is_empty() {
        tooltip_text.push_str(&format!("\n{}", s.ea1c_headline));
    }
    if !s.reading_age_display.is_empty() && !snapshot.timestamp.is_empty() {
        let measured = format!("Measured: {}", snapshot.timestamp);
        tooltip_text = tooltip_text
            .replace(&measured, &format!("{measured} ({})", s.reading_age_display));
    }
    if s.reading_stale && settings.show_glucose_in_tray_tooltip {
        tooltip_text = format!(
            "⚠ STALE — last reading {}\n{tooltip_text}",
            s.reading_age_display
        );
    }
    if s.alerts_snoozed && settings.show_glucose_in_tray_tooltip && !s.snooze_until_display.is_empty() {
        tooltip_text.push_str(&format!("\n{}", s.snooze_until_display));
    }

    let status_header = if s.reading_stale {
        format!(
            "Blood Sugar: {} {} (STALE — {})",
            s.glucose_display, s.result_unit, s.reading_age_display
        )
    } else if snapshot.value_mg_dl > 0.0 {
        format!(
            "Blood Sugar: {} {} {} ({})",
            s.glucose_display, s.result_unit, s.trend_symbol, s.trend_description
        )
    } else {
        "Blood Sugar: --".to_string()
    };
    let range_header = format!("Target Range: {}", s.target_range_display);

    let mut flags: u8 = 0;
    if settings.theme_mode != "light" {
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
    if s.overlay_visible {
        flags |= 0x10;
    }
    if s.overlay_on_all_desktops {
        flags |= 0x20;
    }

    qobject::TrayPayload {
        theme_mode: QString::from(settings.theme_mode.as_str()),
        badge_text: QString::from(badge_text.as_str()),
        trend_symbol: QString::from(s.trend_symbol.as_str()),
        badge_color_hex: QString::from(s.range_color_hex.as_str()),
        status_header: QString::from(status_header.as_str()),
        range_header: QString::from(range_header.as_str()),
        tooltip_text: QString::from(tooltip_text.as_str()),
        flags,
    }
}
