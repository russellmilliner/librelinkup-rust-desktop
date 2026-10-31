//! `SettingsController` implementation: preferences, window/overlay placement, alert
//! configuration, updater *status*, and the diagnostic log viewer.
//!
//! Every setter writes to shared state and persists immediately - there is deliberately no
//! "Save" button anywhere in the UI, so a setting can never be lost by closing the window.

use std::path::Path;
use std::pin::Pin;

use cxx_qt_lib::QString;

use crate::bridge::autostart::sync_autostart_desktop_file;
use crate::bridge::ops;
use crate::bridge::qobject_bridge::{qobject, SettingsControllerRust};
use crate::bridge::state::with_state;
use crate::crypto::save_default_credentials;
use crate::event_log::{
    clear_events, export_events_to_path, get_event_count, get_events_json, log_error, log_info,
};
use crate::models::{MAX_POLL_INTERVAL_SECONDS, MIN_POLL_INTERVAL_SECONDS};

macro_rules! sync_str {
    ($self:ident, $setter:ident, $getter:ident, $value:expr) => {{
        let new_value: String = $value;
        if $self.$getter().to_string() != new_value {
            $self.as_mut().$setter(QString::from(new_value.as_str()));
        }
    }};
}

macro_rules! sync_val {
    ($self:ident, $setter:ident, $getter:ident, $value:expr) => {{
        let new_value = $value;
        if *$self.$getter() != new_value {
            $self.as_mut().$setter(new_value);
        }
    }};
}

impl Default for SettingsControllerRust {
    fn default() -> Self {
        // Valid empty values only; `initialize()` -> `sync()` fills them from shared state.
        Self {
            theme_mode: QString::from("system"),
            ui_zoom_percent: 100,
            high_contrast_mode: false,
            country: QString::from("us"),
            language: QString::from("en"),
            window_mode: QString::from("windowed"),
            overlay_visible: false,
            overlay_only_mode: false,
            overlay_on_all_desktops: true,
            overlay_dark_theme: true,
            overlay_opacity_percent: 88,
            overlay_screen_name: QString::default(),
            overlay_x: -1,
            overlay_y: -1,
            overlay_width: 276,
            overlay_height: 88,
            window_screen_name: QString::default(),
            window_x: -1,
            window_y: -1,
            window_width: 980,
            window_height: 680,
            show_glucose_in_tray_icon: true,
            show_glucose_in_tray_tooltip: true,
            autostart_enabled: true,
            start_minimized: true,
            poll_interval_seconds: MIN_POLL_INTERVAL_SECONDS as i32,
            bring_to_front_enabled: false,
            flash_window_enabled: true,
            audio_alert_enabled: true,
            use_custom_sound: false,
            custom_sound_path: QString::default(),
            override_threshold: false,
            custom_target_low: 70.0,
            custom_target_high: 180.0,
            remember_password: false,
            credentials_vault_path: QString::default(),
            data_source: QString::from("librelinkup"),
            ha_url: QString::default(),
            ha_sensor_entity_id: QString::default(),
            ha_token_configured: false,
            ha_sync_medications: true,
            ha_sync_snooze: true,
            sensor_expiry_alert_hours: 24,
            urgent_low_mg_dl: 55.0,
            alert_repeat_minutes: 15,
            stale_after_minutes: 15,
            alert_on_stale: true,
            check_updates_on_startup: true,
            update_available: false,
            latest_version: QString::from(concat!("v", env!("CARGO_PKG_VERSION"))),
            update_url: QString::from(crate::updater::GITHUB_RELEASES_PAGE_URL),
            update_download_url: QString::from(crate::updater::GITHUB_RELEASES_PAGE_URL),
            update_status_message: QString::from(concat!(
                "Current version: v",
                env!("CARGO_PKG_VERSION")
            )),
            checking_for_updates: false,
            installing_update: false,
            event_log_json: QString::from("[]"),
            event_log_count: 0,
        }
    }
}

impl qobject::SettingsController {
    pub fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().sync();
    }

    /// Mirrors shared state into QML properties (only pushes changed values).
    pub fn sync(mut self: Pin<&mut Self>) {
        let values = with_state(|s| {
            (
                s.theme_mode.clone(),
                s.country.clone(),
                s.language.clone(),
                s.window_mode.clone(),
                s.overlay_visible,
                s.overlay_only_mode,
                s.overlay_on_all_desktops,
                s.overlay_dark_theme,
                s.overlay_opacity_percent,
                s.overlay_screen_name.clone(),
                s.overlay_x,
                s.overlay_y,
                s.overlay_width,
                s.overlay_height,
                s.window_screen_name.clone(),
                s.window_x,
                s.window_y,
                s.window_width,
                s.window_height,
                s.show_glucose_in_tray_icon,
                s.show_glucose_in_tray_tooltip,
                s.autostart_enabled,
                s.start_minimized,
                s.poll_interval_seconds,
                s.bring_to_front_enabled,
                s.flash_window_enabled,
                s.audio_alert_enabled,
                s.use_custom_sound,
                s.custom_sound_path.clone(),
                s.override_threshold,
                s.custom_target_low,
                s.custom_target_high,
                s.remember_password,
                s.credentials_vault_path.clone(),
                s.data_source.clone(),
                s.ha_url.clone(),
                s.ha_sensor_entity_id.clone(),
                s.ha_token_configured,
                s.urgent_low_mg_dl,
                s.alert_repeat_minutes,
                s.stale_after_minutes,
                s.alert_on_stale,
                s.check_updates_on_startup,
                s.update_available,
                s.latest_version.clone(),
                s.update_url.clone(),
                s.update_download_url.clone(),
                s.update_status_message.clone(),
                s.checking_for_updates,
                s.installing_update,
                s.event_log_json.clone(),
                s.event_log_count,
            )
        });
        let (
            theme_mode,
            country,
            language,
            window_mode,
            overlay_visible,
            overlay_only,
            overlay_all_desktops,
            overlay_dark,
            overlay_opacity,
            overlay_screen,
            overlay_x,
            overlay_y,
            overlay_w,
            overlay_h,
            window_screen,
            window_x,
            window_y,
            window_w,
            window_h,
            tray_icon,
            tray_tooltip,
            autostart,
            start_minimized,
            poll_interval,
            bring_to_front,
            flash_window,
            audio_alert,
            use_custom_sound,
            custom_sound_path,
            override_threshold,
            custom_low,
            custom_high,
            remember_password,
            vault_path,
            data_source,
            ha_url,
            ha_sensor_entity_id,
            ha_token_configured,
            urgent_low,
            repeat_minutes,
            stale_after,
            alert_on_stale,
            check_updates,
            update_available,
            latest_version,
            update_url,
            update_download_url,
            update_status,
            checking,
            installing,
            event_json,
            event_count,
        ) = values;

        sync_str!(self, set_theme_mode, theme_mode, theme_mode);
        sync_str!(self, set_country, country, country);
        sync_str!(self, set_language, language, language);
        sync_str!(self, set_window_mode, window_mode, window_mode);
        sync_val!(self, set_overlay_visible, overlay_visible, overlay_visible);
        sync_val!(
            self,
            set_overlay_only_mode,
            overlay_only_mode,
            overlay_only
        );
        sync_val!(
            self,
            set_overlay_on_all_desktops,
            overlay_on_all_desktops,
            overlay_all_desktops
        );
        sync_val!(
            self,
            set_overlay_dark_theme,
            overlay_dark_theme,
            overlay_dark
        );
        sync_val!(
            self,
            set_overlay_opacity_percent,
            overlay_opacity_percent,
            overlay_opacity
        );
        sync_str!(
            self,
            set_overlay_screen_name,
            overlay_screen_name,
            overlay_screen
        );
        sync_val!(self, set_overlay_x, overlay_x, overlay_x);
        sync_val!(self, set_overlay_y, overlay_y, overlay_y);
        sync_val!(self, set_overlay_width, overlay_width, overlay_w);
        sync_val!(self, set_overlay_height, overlay_height, overlay_h);
        sync_str!(
            self,
            set_window_screen_name,
            window_screen_name,
            window_screen
        );
        sync_val!(self, set_window_x, window_x, window_x);
        sync_val!(self, set_window_y, window_y, window_y);
        sync_val!(self, set_window_width, window_width, window_w);
        sync_val!(self, set_window_height, window_height, window_h);
        sync_val!(
            self,
            set_show_glucose_in_tray_icon,
            show_glucose_in_tray_icon,
            tray_icon
        );
        sync_val!(
            self,
            set_show_glucose_in_tray_tooltip,
            show_glucose_in_tray_tooltip,
            tray_tooltip
        );
        sync_val!(self, set_autostart_enabled, autostart_enabled, autostart);
        sync_val!(self, set_start_minimized, start_minimized, start_minimized);
        sync_val!(
            self,
            set_poll_interval_seconds,
            poll_interval_seconds,
            poll_interval
        );
        sync_val!(
            self,
            set_bring_to_front_enabled,
            bring_to_front_enabled,
            bring_to_front
        );
        sync_val!(
            self,
            set_flash_window_enabled,
            flash_window_enabled,
            flash_window
        );
        sync_val!(self, set_audio_alert_enabled, audio_alert_enabled, audio_alert);
        sync_val!(self, set_use_custom_sound, use_custom_sound, use_custom_sound);
        sync_str!(
            self,
            set_custom_sound_path,
            custom_sound_path,
            custom_sound_path
        );
        sync_val!(self, set_override_threshold, override_threshold, override_threshold);
        sync_val!(self, set_custom_target_low, custom_target_low, custom_low);
        sync_val!(self, set_custom_target_high, custom_target_high, custom_high);
        sync_val!(self, set_remember_password, remember_password, remember_password);
        sync_str!(
            self,
            set_credentials_vault_path,
            credentials_vault_path,
            vault_path
        );
        sync_str!(self, set_data_source, data_source, data_source);
        sync_str!(self, set_ha_url, ha_url, ha_url);
        sync_str!(
            self,
            set_ha_sensor_entity_id,
            ha_sensor_entity_id,
            ha_sensor_entity_id
        );
        sync_val!(
            self,
            set_ha_token_configured,
            ha_token_configured,
            ha_token_configured
        );
        sync_val!(self, set_urgent_low_mg_dl, urgent_low_mg_dl, urgent_low);
        sync_val!(
            self,
            set_alert_repeat_minutes,
            alert_repeat_minutes,
            repeat_minutes
        );
        sync_val!(
            self,
            set_stale_after_minutes,
            stale_after_minutes,
            stale_after
        );
        sync_val!(self, set_alert_on_stale, alert_on_stale, alert_on_stale);
        sync_val!(
            self,
            set_check_updates_on_startup,
            check_updates_on_startup,
            check_updates
        );
        sync_val!(self, set_update_available, update_available, update_available);
        sync_str!(self, set_latest_version, latest_version, latest_version);
        sync_str!(self, set_update_url, update_url, update_url);
        sync_str!(
            self,
            set_update_download_url,
            update_download_url,
            update_download_url
        );
        sync_str!(
            self,
            set_update_status_message,
            update_status_message,
            update_status
        );
        sync_val!(
            self,
            set_checking_for_updates,
            checking_for_updates,
            checking
        );
        sync_val!(self, set_installing_update, installing_update, installing);
        sync_str!(self, set_event_log_json, event_log_json, event_json);
        sync_val!(self, set_event_log_count, event_log_count, event_count);
        let (ui_zoom, high_contrast, ha_sync_meds, ha_sync_snz, sensor_exp_hrs) = with_state(|s| {
            (
                s.ui_zoom_percent,
                s.high_contrast_mode,
                s.ha_sync_medications,
                s.ha_sync_snooze,
                s.sensor_expiry_alert_hours,
            )
        });
        sync_val!(self, set_ui_zoom_percent, ui_zoom_percent, ui_zoom);
        sync_val!(
            self,
            set_high_contrast_mode,
            high_contrast_mode,
            high_contrast
        );
        sync_val!(
            self,
            set_ha_sync_medications,
            ha_sync_medications,
            ha_sync_meds
        );
        sync_val!(self, set_ha_sync_snooze, ha_sync_snooze, ha_sync_snz);
        sync_val!(
            self,
            set_sensor_expiry_alert_hours,
            sensor_expiry_alert_hours,
            sensor_exp_hrs
        );
    }

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
        self.as_mut().sync();
    }

    pub fn update_ui_zoom_percent(mut self: Pin<&mut Self>, zoom_percent: i32) {
        let clamped = crate::models::clamp_ui_zoom_percent(zoom_percent);
        with_state(|s| s.ui_zoom_percent = clamped);
        let _ = with_state(|s| ops::persist_settings(s));
        log_info("SYSTEM", format!("Set UI zoom scale to {clamped}%."));
        self.as_mut().sync();
    }

    pub fn zoom_in(mut self: Pin<&mut Self>) {
        let next = with_state(|s| crate::models::clamp_ui_zoom_percent(s.ui_zoom_percent + 10));
        self.as_mut().update_ui_zoom_percent(next);
    }

    pub fn zoom_out(mut self: Pin<&mut Self>) {
        let next = with_state(|s| crate::models::clamp_ui_zoom_percent(s.ui_zoom_percent - 10));
        self.as_mut().update_ui_zoom_percent(next);
    }

    pub fn reset_zoom(mut self: Pin<&mut Self>) {
        self.as_mut().update_ui_zoom_percent(100);
    }

    pub fn update_high_contrast_mode(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.high_contrast_mode = enabled);
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "SYSTEM",
            format!("Set high contrast accessibility mode to {enabled}."),
        );
        self.as_mut().sync();
    }

    pub fn update_data_source(mut self: Pin<&mut Self>, data_source: QString) {
        let clean = if data_source
            .to_string()
            .trim()
            .eq_ignore_ascii_case("home_assistant")
        {
            "home_assistant".to_string()
        } else {
            "librelinkup".to_string()
        };
        with_state(|s| {
            s.data_source = clean.clone();
            if let Some(creds) = s.creds.as_ref() {
                s.logged_in = creds.has_valid_session_for(&s.data_source, &s.ha_url);
            }
        });
        let _ = with_state(|s| ops::persist_settings(s));
        log_info("API", format!("Set telemetry data source to '{clean}'."));
        self.as_mut().sync();
    }

    pub fn update_ha_sync_medications(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.ha_sync_medications = enabled);
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "MEDS",
            format!("Set Home Assistant two-way medication sync to {enabled}."),
        );
        if enabled {
            ops::spawn_ha_medications_push(None);
        }
        self.as_mut().sync();
    }

    pub fn update_ha_sync_snooze(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.ha_sync_snooze = enabled);
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "ALERT",
            format!("Set Home Assistant cross-device alert snooze sync to {enabled}."),
        );
        if enabled {
            ops::spawn_ha_snooze_push();
        }
        self.as_mut().sync();
    }

    pub fn update_sensor_expiry_alert_hours(mut self: Pin<&mut Self>, hours: i32) {
        let clamped = hours.clamp(0, 168);
        with_state(|s| {
            s.sensor_expiry_alert_hours = clamped;
            ops::refresh_display(s);
        });
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "ALERT",
            format!("Set CGM sensor expiry alert threshold to {clamped}h."),
        );
        self.as_mut().sync();
    }

    pub fn update_result_unit(mut self: Pin<&mut Self>, unit: QString) {
        let clean = if unit.to_string().trim().eq_ignore_ascii_case("mmol/L") {
            "mmol/L"
        } else {
            "mg/dL"
        };
        with_state(|s| {
            s.result_unit = clean.to_string();
            ops::refresh_display(s);
            ops::refresh_analytics(s);
        });
        let _ = with_state(|s| ops::persist_settings(s));
        log_info("GLUCOSE", format!("Changed display unit to '{clean}'."));
        self.as_mut().sync();
    }

    pub fn update_country(mut self: Pin<&mut Self>, country: QString) {
        let raw = country.to_string();
        if raw.trim().is_empty() {
            return;
        }
        let clean = crate::api::normalize_country_code(&raw);
        let remember_pw = with_state(|s| s.remember_password);
        let updated = with_state(|s| {
            s.country = clean.clone();
            if let Some(creds) = s.creds.as_mut() {
                if creds.country != clean {
                    creds.country = clean.clone();
                    let _ = save_default_credentials(&creds.sanitized_for_storage(remember_pw));
                }
            }
            s.logged_in
        });
        let _ = with_state(|s| ops::persist_settings(s));
        log_info("API", format!("Updated regional server to '{clean}'."));
        if updated {
            // A region change invalidates the current token; the telemetry controller owns
            // refreshes, so the UI timer picks this up on the next tick.
            with_state(|s| {
                s.active_refresh = None;
                s.snapshot = crate::models::GlucoseSnapshot::default();
            });
        }
        self.as_mut().sync();
    }

    pub fn update_remember_password(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.remember_password = enabled);
        if !enabled {
            let sanitized = with_state(|s| {
                s.creds.as_mut().and_then(|creds| {
                    if creds.password.is_empty() {
                        None
                    } else {
                        *creds = creds.sanitized_for_storage(false);
                        Some(creds.clone())
                    }
                })
            });
            if let Some(creds) = sanitized {
                // Persist without a stored password; the session token is retained.
                let _ = save_default_credentials(&creds);
                log_info(
                    "CRYPTO",
                    "Cleared stored account password from encrypted vault (session token retained).",
                );
            }
        }
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "CRYPTO",
            format!("Updated remember_password setting to {enabled}"),
        );
        self.as_mut().sync();
    }

    pub fn update_window_mode(mut self: Pin<&mut Self>, mode: QString) {
        let clean = mode.to_string().trim().to_string();
        let valid = match clean.as_str() {
            "overlay" | "overlayTransparent" | "windowed" => clean,
            _ => "windowed".to_string(),
        };
        let is_overlay = valid == "overlay" || valid == "overlayTransparent";
        with_state(|s| {
            s.window_mode = valid.clone();
            s.overlay_visible = is_overlay;
            if !is_overlay {
                s.overlay_only_mode = false;
            } else if valid == "overlayTransparent" && s.overlay_opacity_percent > 75 {
                s.overlay_opacity_percent = 55;
            } else if valid == "overlay" && s.overlay_opacity_percent < 75 {
                s.overlay_opacity_percent = 88;
            }
        });
        if is_overlay {
            let (screen, x, y, w, h) = with_state(|s| {
                (
                    s.overlay_screen_name.clone(),
                    s.overlay_x,
                    s.overlay_y,
                    s.overlay_width,
                    s.overlay_height,
                )
            });
            qobject::librelinkup_restore_overlay_placement(
                &QString::from(screen.as_str()),
                x,
                y,
                w,
                h,
            );
        }
        let _ = with_state(|s| ops::persist_settings(s));
        log_info("SYSTEM", format!("Set window mode to '{valid}'."));
        self.as_mut().sync();
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
            qobject::librelinkup_restore_overlay_placement(
                &QString::from(screen.as_str()),
                x,
                y,
                w,
                h,
            );
        }
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "SYSTEM",
            format!("Floating desktop overlay visible: {visible}"),
        );
        self.as_mut().sync();
    }

    pub fn update_overlay_only_mode(mut self: Pin<&mut Self>, only_overlay: bool) {
        with_state(|s| {
            let effective = only_overlay && s.overlay_visible;
            if s.overlay_only_mode != effective {
                s.overlay_only_mode = effective;
                let _ = ops::persist_settings(s);
            }
        });
        self.as_mut().sync();
    }

    pub fn update_overlay_on_all_desktops(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.overlay_on_all_desktops = enabled);
        qobject::librelinkup_apply_overlay_all_desktops(enabled);
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "SYSTEM",
            format!("Desktop overlay on all virtual desktops: {enabled}"),
        );
        self.as_mut().sync();
    }

    pub fn update_overlay_dark_theme(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.overlay_dark_theme = enabled);
        let _ = with_state(|s| ops::persist_settings(s));
        log_info("SYSTEM", format!("Desktop overlay dark theme: {enabled}"));
        self.as_mut().sync();
    }

    pub fn update_overlay_opacity_percent(mut self: Pin<&mut Self>, opacity_percent: i32) {
        let clamped = opacity_percent.clamp(20, 100);
        with_state(|s| s.overlay_opacity_percent = clamped);
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "SYSTEM",
            format!("Desktop overlay opacity set to {clamped}%"),
        );
        self.as_mut().sync();
    }

    pub fn save_overlay_placement(
        mut self: Pin<&mut Self>,
        screen_name: QString,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        let clean_screen = screen_name.to_string().trim().to_string();
        let w = width.clamp(230, 600);
        let h = height.clamp(80, 260);
        let changed = with_state(|s| {
            if s.overlay_screen_name != clean_screen
                || s.overlay_x != x
                || s.overlay_y != y
                || s.overlay_width != w
                || s.overlay_height != h
            {
                if !clean_screen.is_empty() {
                    s.overlay_screen_name = clean_screen.clone();
                }
                s.overlay_x = x;
                s.overlay_y = y;
                s.overlay_width = w;
                s.overlay_height = h;
                true
            } else {
                false
            }
        });
        if changed {
            let _ = with_state(|s| ops::persist_settings(s));
            log_info(
                "SYSTEM",
                format!(
                    "Saved overlay placement: screen='{clean_screen}' pos=({x},{y}) size={w}x{h}"
                ),
            );
            self.as_mut().sync();
        }
    }

    pub fn save_main_window_placement(
        mut self: Pin<&mut Self>,
        screen_name: QString,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        let clean_screen = screen_name.to_string().trim().to_string();
        let w = width.clamp(780, 3840);
        let h = height.clamp(540, 2160);
        let changed = with_state(|s| {
            if s.window_screen_name != clean_screen
                || s.window_x != x
                || s.window_y != y
                || s.window_width != w
                || s.window_height != h
            {
                if !clean_screen.is_empty() {
                    s.window_screen_name = clean_screen.clone();
                }
                s.window_x = x;
                s.window_y = y;
                s.window_width = w;
                s.window_height = h;
                true
            } else {
                false
            }
        });
        if changed {
            let _ = with_state(|s| ops::persist_settings(s));
            self.as_mut().sync();
        }
    }

    pub fn begin_overlay_drag(self: Pin<&mut Self>) {
        qobject::librelinkup_begin_overlay_drag();
    }

    pub fn update_overlay_drag(self: Pin<&mut Self>) {
        qobject::librelinkup_update_overlay_drag();
    }

    pub fn end_overlay_drag(self: Pin<&mut Self>) {
        qobject::librelinkup_end_overlay_drag(self);
    }

    pub fn popup_tray_menu_at_cursor(self: Pin<&mut Self>) {
        qobject::librelinkup_popup_tray_menu();
    }

    pub fn update_show_glucose_in_tray_icon(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.show_glucose_in_tray_icon = enabled);
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "TRAY",
            format!("Show glucose level in tray icon badge: {enabled}"),
        );
        self.as_mut().sync();
    }

    pub fn update_show_glucose_in_tray_tooltip(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.show_glucose_in_tray_tooltip = enabled);
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "TRAY",
            format!("Show glucose telemetry in tray hover tooltip: {enabled}"),
        );
        self.as_mut().sync();
    }

    pub fn update_autostart_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        let minimized = with_state(|s| {
            s.autostart_enabled = enabled;
            s.start_minimized
        });
        sync_autostart_desktop_file(enabled, minimized);
        let _ = with_state(|s| ops::persist_settings(s));
        self.as_mut().sync();
    }

    pub fn update_start_minimized(mut self: Pin<&mut Self>, enabled: bool) {
        let autostart = with_state(|s| {
            s.start_minimized = enabled;
            s.autostart_enabled
        });
        if autostart {
            sync_autostart_desktop_file(autostart, enabled);
        }
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "SYSTEM",
            format!("Start minimized to tray on login: {enabled}"),
        );
        self.as_mut().sync();
    }

    pub fn update_poll_interval_seconds(mut self: Pin<&mut Self>, seconds: i32) {
        let clamped = seconds.clamp(
            MIN_POLL_INTERVAL_SECONDS as i32,
            MAX_POLL_INTERVAL_SECONDS as i32,
        );
        with_state(|s| s.poll_interval_seconds = clamped);
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "API",
            format!("Updated polling interval to {clamped} seconds."),
        );
        self.as_mut().sync();
    }

    #[allow(clippy::too_many_arguments)]
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

        with_state(|s| {
            s.bring_to_front_enabled = bring_to_front;
            s.flash_window_enabled = flash_window;
            s.audio_alert_enabled = audio_alert;
            s.use_custom_sound = use_custom_sound;
            s.custom_sound_path = clean_path.clone();
            s.override_threshold = override_threshold;
            s.custom_target_low = low;
            s.custom_target_high = high;
            ops::refresh_display(s);
            ops::refresh_analytics(s);
        });
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "ALERT",
            format!(
                "Saved alert settings (override={override_threshold}, range={low:.0}-{high:.0} mg/dL, audio={audio_alert})."
            ),
        );
        self.as_mut().sync();
    }

    pub fn test_alert_now(self: Pin<&mut Self>) {
        let settings = with_state(|s| ops::current_settings(s));
        log_info("ALERT", "Triggered manual alert test from Settings.");
        crate::bridge::autostart::fire_alert(
            &settings,
            "LibreLinkUp alert test",
            "This is how out-of-range glucose alerts will look and sound.",
            true,
        );
    }

    pub fn save_advanced_alert_settings(
        mut self: Pin<&mut Self>,
        urgent_low: f64,
        repeat_minutes: i32,
        stale_after_minutes: i32,
        alert_on_stale: bool,
    ) {
        let urgent_low = if urgent_low.is_finite() {
            urgent_low.clamp(40.0, 100.0)
        } else {
            55.0
        };
        let repeat = repeat_minutes.clamp(1, 240);
        let stale = stale_after_minutes.clamp(5, 120);
        with_state(|s| {
            s.urgent_low_mg_dl = urgent_low;
            s.alert_repeat_minutes = repeat;
            s.stale_after_minutes = stale;
            s.alert_on_stale = alert_on_stale;
            ops::refresh_display(s);
        });
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "ALERT",
            format!(
                "Saved advanced alert settings (urgent low={urgent_low:.0} mg/dL, repeat={repeat} min, stale after={stale} min, stale alert={alert_on_stale})."
            ),
        );
        self.as_mut().sync();
    }

    pub fn update_check_updates_on_startup(mut self: Pin<&mut Self>, enabled: bool) {
        with_state(|s| s.check_updates_on_startup = enabled);
        let _ = with_state(|s| ops::persist_settings(s));
        log_info(
            "SYSTEM",
            format!("Updated check_updates_on_startup setting to {enabled}"),
        );
        self.as_mut().sync();
    }

    // ---- diagnostics ----

    pub fn refresh_event_logs(mut self: Pin<&mut Self>) {
        let json = get_events_json();
        let count = get_event_count() as i32;
        with_state(|s| {
            s.event_log_json = json;
            s.event_log_count = count;
        });
        self.as_mut().sync();
    }

    pub fn clear_event_logs(mut self: Pin<&mut Self>) {
        clear_events();
        log_info("SYSTEM", "Event log cleared by user.");
        self.as_mut().refresh_event_logs();
    }

    pub fn export_event_logs(mut self: Pin<&mut Self>, path: QString) -> bool {
        let p = path.to_string();
        match export_events_to_path(Path::new(&p)) {
            Ok(n) => {
                log_info("SYSTEM", format!("Exported {n} log entries to {p}"));
                self.as_mut().refresh_event_logs();
                true
            }
            Err(e) => {
                log_error("SYSTEM", format!("Failed to export logs: {e}"));
                self.as_mut().refresh_event_logs();
                false
            }
        }
    }

    pub fn show_main_window(mut self: Pin<&mut Self>) {
        with_state(|s| {
            s.overlay_only_mode = false;
            let _ = ops::persist_settings(s);
        });
        qobject::librelinkup_show_main_window();
        self.as_mut().sync();
    }

    pub fn hide_main_window(mut self: Pin<&mut Self>) {
        with_state(|s| {
            s.overlay_only_mode = s.overlay_visible;
            let _ = ops::persist_settings(s);
        });
        qobject::librelinkup_hide_main_window();
        self.as_mut().sync();
    }

    pub fn quit_application(self: Pin<&mut Self>) {
        with_state(|s| ops::persist_history_if_due(s, true));
        qobject::librelinkup_quit_app();
    }
}
