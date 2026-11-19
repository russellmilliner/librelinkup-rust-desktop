//! General appearance, overlay/tray settings, Home Assistant sync, and account vault tabs.

use eframe::egui::{self, Color32, RichText};
use librelinkup_core::models::{
    clamp_ui_zoom_percent, MAX_POLL_INTERVAL_SECONDS, MIN_POLL_INTERVAL_SECONDS,
};
use librelinkup_core::paths::credentials_file_path;
use zeroize::{Zeroize, Zeroizing};

use super::{
    clear_text_edit_undoer, region_combo, LibreLinkUpApp, Tab, LOGIN_HA_TOK_EDIT_SALT,
    LOGIN_PW_EDIT_SALT, SETTINGS_HA_TOK_EDIT_SALT,
};
use crate::service::{Command, Shared};

impl LibreLinkUpApp {
    pub(super) fn settings_view(&mut self, ui: &mut egui::Ui, state: &Shared) {
        let mut s = state.settings.clone();
        ui.heading("Settings");
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            egui::Grid::new("settings").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
                ui.label("Theme");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut s.theme_mode, "system".to_string(), "System");
                    ui.selectable_value(&mut s.theme_mode, "light".to_string(), "Light");
                    ui.selectable_value(&mut s.theme_mode, "dark".to_string(), "Dark");
                });
                ui.end_row();
                ui.label("UI Zoom (75%–175%)");
                ui.horizontal(|ui| {
                    let cur_z = clamp_ui_zoom_percent(s.ui_text_scale);
                    if ui.add_enabled(cur_z > 75, egui::Button::new("−")).clicked() {
                        s.ui_text_scale = clamp_ui_zoom_percent(cur_z - 10);
                    }
                    if ui.button(format!("{cur_z}%")).on_hover_text("Reset UI zoom to 100%").clicked() {
                        s.ui_text_scale = 100;
                    }
                    if ui.add_enabled(cur_z < 175, egui::Button::new("+")).clicked() {
                        s.ui_text_scale = clamp_ui_zoom_percent(cur_z + 10);
                    }
                    ui.checkbox(&mut s.high_contrast_mode, "High Contrast Mode");
                });
                ui.end_row();
                ui.label("Glucose unit");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut s.result_unit, "mg/dL".to_string(), "mg/dL");
                    ui.selectable_value(&mut s.result_unit, "mmol/L".to_string(), "mmol/L");
                });
                ui.end_row();
                ui.label("Refresh every");
                ui.add(
                    egui::DragValue::new(&mut s.poll_interval_seconds)
                        .range(MIN_POLL_INTERVAL_SECONDS..=MAX_POLL_INTERVAL_SECONDS)
                        .suffix(" s"),
                );
                ui.end_row();
                ui.label("Regional endpoint");
                region_combo(ui, &mut s.country);
                ui.end_row();
            });
        });
        ui.add_space(8.0);
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.label(RichText::new("System tray").strong());
            ui.checkbox(&mut s.show_glucose_in_tray_icon, "Show glucose value in the tray icon");
            ui.checkbox(&mut s.show_glucose_in_tray_tooltip, "Show glucose details in the tray tooltip");
            ui.checkbox(&mut s.autostart_enabled, "Start at Windows sign-in");
            ui.add_enabled(s.autostart_enabled, egui::Checkbox::new(&mut s.start_minimized, "Start minimized to the tray"));
        });
        ui.add_space(8.0);
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.label(RichText::new("Floating overlay").strong());
            ui.checkbox(&mut s.overlay_visible, "Show floating desktop overlay (always on top)");
            ui.checkbox(&mut s.overlay_on_all_desktops, "Show overlay on all virtual desktops");
            ui.checkbox(&mut s.overlay_dark_theme, "Dark overlay theme");
            ui.horizontal(|ui| {
                ui.label("Opacity");
                ui.add(egui::Slider::new(&mut s.overlay_opacity_percent, 20..=100).suffix("%"));
            });
            ui.label(RichText::new("Drag the overlay to move it. Scroll wheel adjusts opacity. Right-click or use buttons for options.").small().weak());
        });
        ui.add_space(8.0);
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.label(RichText::new("Application updates").strong());
            ui.checkbox(
                &mut s.check_updates_on_startup,
                "Automatically check GitHub Releases for updates (on startup & every 6 hours)",
            );
                ui.horizontal(|ui| {
                    let btn_label = if state.checking_updates {
                        "Checking..."
                    } else {
                        "🔄 Check for Updates Now"
                    };
                    if ui
                        .add_enabled(!state.checking_updates, egui::Button::new(btn_label))
                        .clicked()
                    {
                        self.send(Command::CheckUpdates { manual: true });
                    }
                    if let Some(ref upd) = state.update_info {
                        let color = if upd.update_available {
                            Color32::from_rgb(56, 189, 248)
                        } else {
                            ui.visuals().text_color()
                        };
                        ui.label(RichText::new(&upd.message).color(color));
                        if upd.update_available {
                            let inst_label = if state.installing_update {
                                "Installing...".to_string()
                            } else {
                                format!("⬇ Install {}", upd.latest_version)
                            };
                            if ui
                                .add_enabled(!state.installing_update, egui::Button::new(inst_label))
                                .clicked()
                            {
                                self.send(Command::InstallUpdate);
                            }
                            ui.hyperlink_to("Release Notes", &upd.release_url);
                        }
                    } else {
                        ui.label(RichText::new(format!("Current version: v{}", env!("CARGO_PKG_VERSION"))).weak());
                    }
                });
            });
        ui.add_space(8.0);
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.label(RichText::new("Data Source & Home Assistant LibreLink Integration").strong());
            ui.label(
                RichText::new(
                    "Choose whether to poll Abbott LibreLinkUp Cloud directly or pull CGM readings and 12-hour history from your Home Assistant LibreLink integration.",
                )
                .small()
                .weak(),
            );
            if self.ha_url_input.is_empty() && !s.ha_url.is_empty() {
                self.ha_url_input = s.ha_url.clone();
            }
            if self.ha_entity_input.is_empty() && !s.ha_sensor_entity_id.is_empty() {
                self.ha_entity_input = s.ha_sensor_entity_id.clone();
            }
            egui::Grid::new("ha_settings_grid")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Telemetry source");
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut s.data_source,
                            "librelinkup".to_string(),
                            "LibreLinkUp Cloud API",
                        );
                        ui.selectable_value(
                            &mut s.data_source,
                            "home_assistant".to_string(),
                            "Home Assistant (LibreLink)",
                        );
                    });
                    ui.end_row();
                    ui.label("Home Assistant URL");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.ha_url_input)
                            .hint_text("http://homeassistant.local:8123")
                            .desired_width(300.0),
                    );
                    ui.end_row();
                    ui.label("Long-Lived Access Token");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.ha_token_input)
                            .id(egui::Id::unique(SETTINGS_HA_TOK_EDIT_SALT))
                            .password(true)
                            .hint_text(if state.ha_token_configured {
                                "Stored in DPAPI vault (leave blank to keep)"
                            } else {
                                "Paste Home Assistant Long-Lived Access Token"
                            })
                            .desired_width(300.0),
                    );
                    ui.end_row();
                    ui.label("Patient / Sensor");
                    let selected_label = state
                        .discovered_ha_patients
                        .iter()
                        .find(|p| p.entity_id.eq_ignore_ascii_case(self.ha_entity_input.trim()))
                        .map(|p| p.label.as_str())
                        .unwrap_or("Auto-discover (First Active Sensor)");
                    egui::ComboBox::from_id_salt("settings_ha_patient_combo")
                        .selected_text(selected_label)
                        .width(300.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    self.ha_entity_input.trim().is_empty(),
                                    "Auto-discover (First Active Sensor)",
                                )
                                .clicked()
                            {
                                self.ha_entity_input.clear();
                                s.ha_sensor_entity_id.clear();
                                self.send(Command::SelectHaPatient(String::new()));
                            }
                            for p in &state.discovered_ha_patients {
                                let is_sel =
                                    p.entity_id.eq_ignore_ascii_case(self.ha_entity_input.trim());
                                if ui.selectable_label(is_sel, &p.label).clicked() {
                                    self.ha_entity_input = p.entity_id.clone();
                                    s.ha_sensor_entity_id = p.entity_id.clone();
                                    self.send(Command::SelectHaPatient(p.entity_id.clone()));
                                }
                            }
                        });
                    ui.end_row();
                    ui.label("Sensor Entity ID (optional)");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.ha_entity_input)
                            .hint_text("Auto-discover (e.g. sensor.patient_glucose_measurement)")
                            .desired_width(300.0),
                    );
                    ui.end_row();
                });
            ui.checkbox(
                &mut s.ha_sync_medications,
                "Two-way Medication & Dose Sync with Home Assistant (sensor.librelinkup_medications)",
            );
            ui.checkbox(
                &mut s.ha_sync_snooze,
                "Cross-Device Alert Snooze Synchronization via Home Assistant (sensor.librelinkup_alert_snooze)",
            );
            ui.horizontal(|ui| {
                if ui.button("💾 Save & Connect Home Assistant").clicked() {
                    let tok = Zeroizing::new(std::mem::take(&mut self.ha_token_input));
                    clear_text_edit_undoer(ui.ctx(), egui::Id::unique(SETTINGS_HA_TOK_EDIT_SALT));
                    self.send(Command::SaveHomeAssistantConfig {
                        data_source: s.data_source.clone(),
                        ha_url: self.ha_url_input.clone(),
                        ha_token: tok,
                        ha_sensor_entity_id: self.ha_entity_input.clone(),
                    });
                }
                if ui.button("📥 Import ~/.config/home-assistant.env").clicked() {
                    self.send(Command::AutoFillHomeAssistantFromEnv);
                }
            });
        });
        if s != state.settings {
            self.save_settings(s);
        }
    }

    pub(super) fn account_view(&mut self, ui: &mut egui::Ui, state: &Shared) {
        ui.heading("Account & Encrypted Credential Vault");
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            egui::Grid::new("account").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                ui.label("Session status");
                ui.label(&state.status);
                ui.end_row();
                ui.label("Account");
                ui.label(if state.username.is_empty() { "—" } else { &state.username });
                ui.end_row();
                let name = format!("{} {}", state.snapshot.first_name, state.snapshot.last_name).trim().to_string();
                ui.label("Connected patient");
                ui.label(if name.is_empty() { "—".to_string() } else { name });
                ui.end_row();
                ui.label("Region");
                ui.label(state.settings.country.to_uppercase());
                ui.end_row();
                ui.label("Encryption");
                ui.label("AES-256-GCM; key protected by Windows DPAPI (current user)");
                ui.end_row();
                ui.label("Vault path");
                ui.label(credentials_file_path().display().to_string());
                ui.end_row();
            });
            ui.add_space(8.0);
            let mut remember = state.settings.remember_password;
            if ui
                .checkbox(
                    &mut remember,
                    "Remember password in encrypted vault (auto-renew expired tokens)",
                )
                .changed()
            {
                let mut ns = state.settings.clone();
                ns.remember_password = remember;
                self.save_settings(ns);
            }
            ui.add_space(10.0);
            if ui.add_enabled(state.logged_in, egui::Button::new("Sign Out & Wipe Encrypted Vault")).clicked() {
                self.login_password.zeroize();
                self.ha_token_input.zeroize();
                clear_text_edit_undoer(ui.ctx(), egui::Id::unique(LOGIN_PW_EDIT_SALT));
                clear_text_edit_undoer(ui.ctx(), egui::Id::unique(LOGIN_HA_TOK_EDIT_SALT));
                clear_text_edit_undoer(ui.ctx(), egui::Id::unique(SETTINGS_HA_TOK_EDIT_SALT));
                self.send(Command::Logout);
                self.tab = Tab::Dashboard;
            }
        });
    }
}
