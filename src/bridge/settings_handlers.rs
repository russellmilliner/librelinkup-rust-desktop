//! Settings synchronization, tray/UI state propagation, alert evaluation, and updater handlers.
#![allow(clippy::too_many_arguments)]

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use crate::api::normalize_country_code;
use crate::bridge::app_controller::qobject;
use crate::bridge::autostart::{
    background_runtime, fire_alert, now_epoch, sync_autostart_desktop_file,
};
use crate::crypto::save_default_credentials;
use crate::event_log::{get_event_count, get_events_json, log_error, log_info, log_warn};
use crate::models::{
    clamp_poll_interval, format_glucose_value, format_reading_age, format_target_range,
    parse_notification_snooze_action, reading_age_minutes, save_default_settings,
    trend_arrow_description, trend_arrow_symbol, AppSettings, GlucoseRangeStatus,
    MAX_POLL_INTERVAL_SECONDS, MIN_POLL_INTERVAL_SECONDS, STALE_COLOR_HEX,
};
use crate::session::{evaluate_alert_decision, format_snooze_until};
use crate::updater::{
    check_for_updates_async, download_and_install_update_async, UpdateCheckResult,
};

pub(crate) fn current_settings_from_controller(ctrl: &qobject::AppController) -> AppSettings {
    AppSettings {
        theme_mode: ctrl.theme_mode().to_string(),
        result_unit: ctrl.result_unit().to_string(),
        country: ctrl.country().to_string(),
        language: ctrl.language().to_string(),
        window_mode: ctrl.window_mode().to_string(),
        overlay_visible: *ctrl.overlay_visible(),
        overlay_only_mode: *ctrl.overlay_only_mode(),
        overlay_on_all_desktops: *ctrl.overlay_on_all_desktops(),
        overlay_dark_theme: *ctrl.overlay_dark_theme(),
        overlay_opacity_percent: (*ctrl.overlay_opacity_percent()).clamp(20, 100),
        overlay_screen_name: ctrl.overlay_screen_name().to_string(),
        overlay_x: *ctrl.overlay_x(),
        overlay_y: *ctrl.overlay_y(),
        overlay_width: (*ctrl.overlay_width()).clamp(230, 600),
        overlay_height: (*ctrl.overlay_height()).clamp(80, 260),
        show_glucose_in_tray_icon: *ctrl.show_glucose_in_tray_icon(),
        show_glucose_in_tray_tooltip: *ctrl.show_glucose_in_tray_tooltip(),
        tray_icon_theme: "auto".to_string(),
        autostart_enabled: *ctrl.autostart_enabled(),
        start_minimized: *ctrl.start_minimized(),
        poll_interval_seconds: clamp_poll_interval(
            (*ctrl.poll_interval_seconds()).max(MIN_POLL_INTERVAL_SECONDS as i32) as u32,
        ),
        bring_to_front_enabled: *ctrl.bring_to_front_enabled(),
        flash_window_enabled: *ctrl.flash_window_enabled(),
        audio_alert_enabled: *ctrl.audio_alert_enabled(),
        use_custom_sound: *ctrl.use_custom_sound(),
        custom_sound_path: ctrl.custom_sound_path().to_string(),
        override_threshold: *ctrl.override_threshold(),
        custom_target_low: *ctrl.custom_target_low(),
        custom_target_high: *ctrl.custom_target_high(),
        urgent_low_mg_dl: *ctrl.urgent_low_mg_dl(),
        alert_repeat_minutes: (*ctrl.alert_repeat_minutes()).clamp(1, 240) as u32,
        stale_after_minutes: (*ctrl.stale_after_minutes()).clamp(5, 120) as u32,
        alert_on_stale: *ctrl.alert_on_stale(),
        snooze_until_epoch: ctrl.rust().snooze_until_epoch,
        check_updates_on_startup: *ctrl.check_updates_on_startup(),
        remember_password: *ctrl.remember_password(),
        ui_text_scale: 100,
    }
}

impl qobject::AppController {
    pub(crate) fn sync_logs_to_properties(mut self: Pin<&mut Self>) {
        let json = get_events_json();
        let count = get_event_count() as i32;
        self.as_mut()
            .set_event_log_json(QString::from(json.as_str()));
        self.as_mut().set_event_log_count(count);
    }

    pub(crate) fn sync_tray_and_ui(mut self: Pin<&mut Self>) {
        let settings = current_settings_from_controller(&self);
        let snapshot = self.rust().snapshot.clone();

        let eff_low = settings.effective_target_low(snapshot.target_low_mg_dl);
        let eff_high = settings.effective_target_high(snapshot.target_high_mg_dl);
        let val_display = format_glucose_value(snapshot.value_mg_dl, &settings.result_unit);
        let arrow_sym = trend_arrow_symbol(snapshot.trend_arrow);
        let arrow_desc = trend_arrow_description(snapshot.trend_arrow);
        let range_str = format_target_range(eff_low, eff_high, &settings.result_unit);

        let now = now_epoch();
        let age_minutes = reading_age_minutes(snapshot.measured_at_epoch, now);
        let age_text = format_reading_age(age_minutes);
        let stale = snapshot.value_mg_dl > 0.0 && settings.is_reading_stale(age_minutes);

        let (status_label, color_hex) = if stale {
            ("Stale", STALE_COLOR_HEX)
        } else if snapshot.value_mg_dl > 0.0 {
            let st = GlucoseRangeStatus::classify(snapshot.value_mg_dl, eff_low, eff_high);
            (st.label(), st.color_hex())
        } else {
            ("Waiting", "#22c55e")
        };

        self.as_mut()
            .set_reading_age_display(QString::from(age_text.as_str()));
        self.as_mut().set_reading_stale(stale);
        let snoozed = settings.is_snoozed(now);
        self.as_mut().set_alerts_snoozed(snoozed);
        self.as_mut().set_snooze_until_display(QString::from(
            format_snooze_until(settings.snooze_until_epoch, now).as_str(),
        ));
        if *self.logged_in() {
            let current_status = self.status_message().to_string();
            if stale {
                self.as_mut().set_status_message(QString::from(
                    format!("No new sensor readings ({age_text}) — check sensor/phone").as_str(),
                ));
            } else if current_status.starts_with("No new sensor readings") {
                self.as_mut()
                    .set_status_message(QString::from("Live CGM telemetry active"));
            }
        }

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
            let updated = if age_text.is_empty() {
                snapshot.timestamp.clone()
            } else {
                format!("{} ({age_text})", snapshot.timestamp)
            };
            self.as_mut()
                .set_last_updated(QString::from(updated.as_str()));
        }

        let points_json =
            serde_json::to_string(&snapshot.graph_points).unwrap_or_else(|_| "[]".to_string());
        self.as_mut()
            .set_graph_points_json(QString::from(points_json.as_str()));

        let badge_text = settings.format_tray_icon_badge(&snapshot);
        let mut tooltip_text = settings.format_tray_hover_tooltip(&snapshot);
        if !age_text.is_empty() && !snapshot.timestamp.is_empty() {
            let measured = format!("Measured: {}", snapshot.timestamp);
            tooltip_text = tooltip_text.replace(&measured, &format!("{measured} ({age_text})"));
        }
        if stale && settings.show_glucose_in_tray_tooltip {
            tooltip_text = format!("⚠ STALE — last reading {age_text}\n{tooltip_text}");
        }
        if snoozed && settings.show_glucose_in_tray_tooltip {
            tooltip_text.push_str(&format!(
                "\n{}",
                format_snooze_until(settings.snooze_until_epoch, now)
            ));
        }
        let status_header = if stale {
            format!(
                "Blood Sugar: {} {} (STALE — {age_text})",
                val_display, settings.result_unit
            )
        } else if snapshot.value_mg_dl > 0.0 {
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
        if *self.overlay_on_all_desktops() {
            flags |= 0x20;
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

    /// Fires an alert when warranted by the current reading/staleness, honoring snooze,
    /// the repeat interval, and escalation (see `evaluate_alert_decision`).
    pub(crate) fn evaluate_alerts(mut self: Pin<&mut Self>) {
        if !*self.logged_in() {
            return;
        }
        let settings = current_settings_from_controller(&self);
        let snap = self.rust().snapshot.clone();
        let now = now_epoch();

        let (decision, cleared_previous) =
            evaluate_alert_decision(&settings, &snap, self.rust().last_alert, now);
        if cleared_previous {
            log_info("ALERT", "Glucose back within alert thresholds.");
            self.as_mut().rust_mut().last_alert = None;
        }

        if let Some(alert) = decision {
            log_warn("ALERT", format!("{}: {}", alert.title, alert.body));
            fire_alert(&settings, alert.title, &alert.body, alert.critical);
            self.as_mut().rust_mut().last_alert = Some((alert.kind, now));
        }
    }

    pub fn evaluate_staleness(mut self: Pin<&mut Self>) {
        self.as_mut().sync_tray_and_ui();
        self.as_mut().evaluate_alerts();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn snooze_alerts(mut self: Pin<&mut Self>, minutes: i32) {
        let now = now_epoch();
        let until = if minutes > 0 {
            now + i64::from(minutes.min(24 * 60)) * 60
        } else {
            0
        };
        self.as_mut().rust_mut().snooze_until_epoch = until;
        if until > 0 {
            log_info(
                "ALERT",
                format!("Alerts snoozed for {minutes} min (urgent low still alerts)."),
            );
        } else {
            log_info("ALERT", "Alert snooze cancelled.");
            // Re-arm so a still-active condition alerts on the next evaluation.
            self.as_mut().rust_mut().last_alert = None;
        }
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn handle_notification_action(self: Pin<&mut Self>, action_key: QString) -> bool {
        let key = action_key.to_string();
        if let Some(minutes) = parse_notification_snooze_action(&key) {
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
        self.as_mut().set_urgent_low_mg_dl(urgent_low);
        self.as_mut()
            .set_alert_repeat_minutes(repeat_minutes.clamp(1, 240));
        self.as_mut()
            .set_stale_after_minutes(stale_after_minutes.clamp(5, 120));
        self.as_mut().set_alert_on_stale(alert_on_stale);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info(
            "ALERT",
            format!(
                "Saved advanced alert settings (urgent low={urgent_low:.0} mg/dL, repeat={} min, stale after={} min, stale alert={alert_on_stale}).",
                settings.alert_repeat_minutes, settings.stale_after_minutes
            ),
        );
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
        let raw = country.to_string();
        if !raw.trim().is_empty() {
            let clean = normalize_country_code(&raw);
            let remember_pw = *self.remember_password();
            self.as_mut().set_country(QString::from(clean.as_str()));
            if let Some(ref mut creds) = self.as_mut().rust_mut().creds {
                if creds.country != clean {
                    creds.country = clean.clone();
                    let _ = save_default_credentials(&creds.sanitized_for_storage(remember_pw));
                }
            }
            let settings = current_settings_from_controller(&self);
            let _ = save_default_settings(&settings);
            log_info("API", format!("Updated regional server to '{clean}'."));
            if *self.logged_in() {
                self.as_mut().refresh_glucose_now();
            }
            self.as_mut().sync_logs_to_properties();
        }
    }

    pub fn update_remember_password(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_remember_password(enabled);
        if !enabled {
            if let Some(ref mut creds) = self.as_mut().rust_mut().creds {
                if !creds.password.is_empty() {
                    *creds = creds.sanitized_for_storage(false);
                    let _ = save_default_credentials(creds);
                    log_info(
                        "CRYPTO",
                        "Cleared stored account password from encrypted vault (session token retained).",
                    );
                }
            }
        }
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info(
            "CRYPTO",
            format!("Updated remember_password setting to {enabled}"),
        );
        self.as_mut().sync_logs_to_properties();
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
        if !is_overlay {
            self.as_mut().set_overlay_only_mode(false);
        } else {
            let ov_screen = self.overlay_screen_name().clone();
            let ov_x = *self.overlay_x();
            let ov_y = *self.overlay_y();
            let ov_w = *self.overlay_width();
            let ov_h = *self.overlay_height();
            qobject::librelinkup_restore_overlay_placement(&ov_screen, ov_x, ov_y, ov_w, ov_h);
        }
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
            self.as_mut().set_overlay_only_mode(false);
        }
        if visible {
            let ov_screen = self.overlay_screen_name().clone();
            let ov_x = *self.overlay_x();
            let ov_y = *self.overlay_y();
            let ov_w = *self.overlay_width();
            let ov_h = *self.overlay_height();
            qobject::librelinkup_restore_overlay_placement(&ov_screen, ov_x, ov_y, ov_w, ov_h);
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

    pub fn update_overlay_only_mode(mut self: Pin<&mut Self>, only_overlay: bool) {
        let effective = only_overlay && *self.overlay_visible();
        if *self.overlay_only_mode() != effective {
            self.as_mut().set_overlay_only_mode(effective);
            let settings = current_settings_from_controller(&self);
            let _ = save_default_settings(&settings);
            self.as_mut().sync_tray_and_ui();
        }
    }

    pub fn update_overlay_on_all_desktops(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_overlay_on_all_desktops(enabled);
        qobject::librelinkup_apply_overlay_all_desktops(enabled);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info(
            "SYSTEM",
            format!("Desktop overlay on all virtual desktops: {enabled}"),
        );
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_overlay_dark_theme(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_overlay_dark_theme(enabled);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info("SYSTEM", format!("Desktop overlay dark theme: {enabled}"));
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
        let changed = self.overlay_screen_name().to_string() != clean_screen
            || *self.overlay_x() != x
            || *self.overlay_y() != y
            || *self.overlay_width() != w
            || *self.overlay_height() != h;

        if changed {
            if !clean_screen.is_empty() {
                self.as_mut()
                    .set_overlay_screen_name(QString::from(clean_screen.as_str()));
            }
            self.as_mut().set_overlay_x(x);
            self.as_mut().set_overlay_y(y);
            self.as_mut().set_overlay_width(w);
            self.as_mut().set_overlay_height(h);
            let settings = current_settings_from_controller(&self);
            let _ = save_default_settings(&settings);
            log_info(
                "SYSTEM",
                format!(
                    "Saved overlay placement: screen='{clean_screen}' pos=({x},{y}) size={w}x{h}"
                ),
            );
            self.as_mut().sync_logs_to_properties();
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
        log_info(
            "SYSTEM",
            format!("Start minimized to tray on login: {enabled}"),
        );
        self.as_mut().sync_tray_and_ui();
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_poll_interval_seconds(mut self: Pin<&mut Self>, seconds: i32) {
        let clamped = seconds.clamp(
            MIN_POLL_INTERVAL_SECONDS as i32,
            MAX_POLL_INTERVAL_SECONDS as i32,
        );
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
        fire_alert(
            &settings,
            "LibreLinkUp alert test",
            "This is how out-of-range glucose alerts will look and sound.",
            true,
        );
        self.as_mut().sync_logs_to_properties();
    }

    pub fn check_for_updates(mut self: Pin<&mut Self>) {
        self.as_mut().start_update_check(true);
    }

    pub fn check_for_updates_silent(mut self: Pin<&mut Self>) {
        if *self.check_updates_on_startup()
            && std::env::var_os("QT_QPA_PLATFORM").as_deref()
                != Some(std::ffi::OsStr::new("offscreen"))
        {
            self.as_mut().start_update_check(false);
        }
    }

    pub fn install_update(mut self: Pin<&mut Self>) {
        if *self.installing_update() {
            return;
        }
        let download_url = self.update_download_url().to_string();
        let latest_ver = self.latest_version().to_string();
        let Some(runtime) = background_runtime() else {
            return;
        };
        self.as_mut().set_installing_update(true);
        self.as_mut().set_update_status_message(QString::from(
            format!("Downloading and installing {latest_ver}...").as_str(),
        ));
        log_info(
            "SYSTEM",
            format!("Downloading and installing update {latest_ver} from {download_url}..."),
        );
        self.as_mut().sync_logs_to_properties();

        let qt_thread = self.qt_thread();
        runtime.spawn(async move {
            let outcome = download_and_install_update_async(&download_url, &latest_ver).await;
            let _ = qt_thread.queue(move |qobject| {
                qobject.apply_install_update_outcome(outcome);
            });
        });
    }

    pub(crate) fn apply_install_update_outcome(
        mut self: Pin<&mut Self>,
        outcome: Result<std::path::PathBuf, String>,
    ) {
        self.as_mut().set_installing_update(false);
        match outcome {
            Ok(path) => {
                let msg = format!(
                    "Update downloaded ({}) — installing and restarting...",
                    path.display()
                );
                self.as_mut()
                    .set_update_status_message(QString::from(msg.as_str()));
                log_info("SYSTEM", msg);
            }
            Err(err) => {
                self.as_mut()
                    .set_update_status_message(QString::from(err.as_str()));
                log_error("SYSTEM", format!("Auto-update install failed: {err}"));
            }
        }
        self.as_mut().sync_logs_to_properties();
    }

    pub fn update_check_updates_on_startup(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_check_updates_on_startup(enabled);
        let settings = current_settings_from_controller(&self);
        let _ = save_default_settings(&settings);
        log_info(
            "SYSTEM",
            format!("Updated check_updates_on_startup setting to {enabled}"),
        );
        self.as_mut().sync_logs_to_properties();
    }

    pub(crate) fn start_update_check(mut self: Pin<&mut Self>, manual: bool) {
        if *self.checking_for_updates() {
            return;
        }
        let Some(runtime) = background_runtime() else {
            return;
        };
        self.as_mut().set_checking_for_updates(true);
        self.as_mut()
            .set_update_status_message(QString::from("Checking GitHub Releases for updates..."));
        if manual {
            log_info(
                "SYSTEM",
                "Checking GitHub Releases for application updates...",
            );
            self.as_mut().sync_logs_to_properties();
        }

        let current_version = env!("CARGO_PKG_VERSION").to_string();
        let qt_thread = self.qt_thread();
        runtime.spawn(async move {
            let outcome = check_for_updates_async(&current_version).await;
            let _ = qt_thread.queue(move |qobject| {
                qobject.apply_update_check_outcome(manual, outcome);
            });
        });
    }

    pub(crate) fn apply_update_check_outcome(
        mut self: Pin<&mut Self>,
        manual: bool,
        outcome: Result<UpdateCheckResult, String>,
    ) {
        self.as_mut().set_checking_for_updates(false);
        self.as_mut().rust_mut().last_update_check_epoch = now_epoch();
        match outcome {
            Ok(res) => {
                self.as_mut().set_update_available(res.update_available);
                self.as_mut()
                    .set_latest_version(QString::from(res.latest_version.as_str()));
                self.as_mut()
                    .set_update_url(QString::from(res.release_url.as_str()));
                self.as_mut()
                    .set_update_download_url(QString::from(res.download_url.as_str()));
                self.as_mut()
                    .set_update_status_message(QString::from(res.message.as_str()));
                if res.update_available {
                    let already_notified =
                        self.rust().last_notified_update_version == res.latest_version;
                    log_info(
                        "SYSTEM",
                        format!(
                            "Update available: {} (current {}). Release page: {}",
                            res.latest_version, res.current_version, res.release_url
                        ),
                    );
                    if manual || !already_notified {
                        self.as_mut().rust_mut().last_notified_update_version =
                            res.latest_version.clone();
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
                self.as_mut()
                    .set_update_status_message(QString::from(err.as_str()));
                if manual {
                    log_warn("SYSTEM", format!("Update check failed: {err}"));
                }
            }
        }
        self.as_mut().sync_logs_to_properties();
    }
}
