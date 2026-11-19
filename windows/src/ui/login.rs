//! Unauthenticated LibreLinkUp & Home Assistant sign-in view.

use eframe::egui::{self, RichText};
use zeroize::Zeroizing;

use super::{
    clear_text_edit_undoer, region_combo, LibreLinkUpApp, LOGIN_HA_TOK_EDIT_SALT,
    LOGIN_PW_EDIT_SALT,
};
use crate::service::{Command, Shared};

impl LibreLinkUpApp {
    pub(super) fn login_view(&mut self, ui: &mut egui::Ui, state: &Shared) {
        let is_ha = state.settings.data_source == "home_assistant";
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.heading(if is_ha {
                "Connect to Home Assistant LibreLink"
            } else {
                "Sign in to LibreLinkUp"
            });
            ui.label(if is_ha {
                "Pull live CGM readings, trend arrows, and 12-hour history from your Home Assistant LibreLink integration."
            } else {
                "Use the email and password of your LibreLinkUp follower account."
            });
            ui.add_space(12.0);
            egui::Frame::group(ui.style()).inner_margin(16).show(ui, |ui| {
                ui.set_max_width(450.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Data Source:").strong());
                    let mut src = state.settings.data_source.clone();
                    if ui
                        .selectable_value(&mut src, "librelinkup".to_string(), "LibreLinkUp Cloud")
                        .clicked()
                        || ui
                            .selectable_value(
                                &mut src,
                                "home_assistant".to_string(),
                                "Home Assistant (LibreLink)",
                            )
                            .clicked()
                    {
                        let mut ns = state.settings.clone();
                        ns.data_source = src;
                        self.save_settings(ns);
                    }
                });
                ui.separator();

                if !is_ha {
                    egui::Grid::new("login").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                        ui.label("Email");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.login_email)
                                .hint_text("you@example.com")
                                .desired_width(260.0),
                        );
                        ui.end_row();
                        ui.label("Password");
                        let pw = ui.add(
                            egui::TextEdit::singleline(&mut self.login_password)
                                .id(egui::Id::unique(LOGIN_PW_EDIT_SALT))
                                .password(true)
                                .desired_width(260.0),
                        );
                        ui.end_row();
                        ui.label("Region");
                        region_combo(ui, &mut self.login_region);
                        ui.end_row();
                        if pw.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.submit_login(ui.ctx());
                        }
                    });
                    ui.add_space(6.0);
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
                    ui.horizontal(|ui| {
                        let label = if state.busy { "Signing in..." } else { "Sign In" };
                        if ui.add_enabled(!state.busy, egui::Button::new(label)).clicked() {
                            self.submit_login(ui.ctx());
                        }
                        if ui.button("Sample Data").clicked() {
                            self.send(Command::LoadDemo);
                        }
                    });
                } else {
                    if self.ha_url_input.is_empty() && !state.settings.ha_url.is_empty() {
                        self.ha_url_input = state.settings.ha_url.clone();
                    }
                    if self.ha_entity_input.is_empty() && !state.settings.ha_sensor_entity_id.is_empty() {
                        self.ha_entity_input = state.settings.ha_sensor_entity_id.clone();
                    }
                    egui::Grid::new("login_ha").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                        ui.label("HA URL");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.ha_url_input)
                                .hint_text("http://homeassistant.local:8123")
                                .desired_width(280.0),
                        );
                        ui.end_row();
                        ui.label("Access Token");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.ha_token_input)
                                .id(egui::Id::unique(LOGIN_HA_TOK_EDIT_SALT))
                                .password(true)
                                .hint_text(if state.ha_token_configured {
                                    "Stored in DPAPI vault (leave blank to keep)"
                                } else {
                                    "Paste Long-Lived Access Token"
                                })
                                .desired_width(280.0),
                        );
                        ui.end_row();
                        if !state.discovered_ha_patients.is_empty() {
                            ui.label("Patient / Sensor");
                            let selected_label = state
                                .discovered_ha_patients
                                .iter()
                                .find(|p| p.entity_id.eq_ignore_ascii_case(self.ha_entity_input.trim()))
                                .map(|p| p.label.as_str())
                                .unwrap_or("Auto-discover (First Active Sensor)");
                            egui::ComboBox::from_id_salt("login_ha_patient_combo")
                                .selected_text(selected_label)
                                .width(280.0)
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(
                                            self.ha_entity_input.trim().is_empty(),
                                            "Auto-discover (First Active Sensor)",
                                        )
                                        .clicked()
                                    {
                                        self.ha_entity_input.clear();
                                        self.send(Command::SelectHaPatient(String::new()));
                                    }
                                    for p in &state.discovered_ha_patients {
                                        let is_sel =
                                            p.entity_id.eq_ignore_ascii_case(self.ha_entity_input.trim());
                                        if ui.selectable_label(is_sel, &p.label).clicked() {
                                            self.ha_entity_input = p.entity_id.clone();
                                            self.send(Command::SelectHaPatient(p.entity_id.clone()));
                                        }
                                    }
                                });
                            ui.end_row();
                        }
                        ui.label("Sensor Entity ID");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.ha_entity_input)
                                .hint_text("Auto-discover (e.g. sensor.patient_glucose_measurement)")
                                .desired_width(280.0),
                        );
                        ui.end_row();
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let label = if state.busy {
                            "Connecting..."
                        } else {
                            "Connect Home Assistant"
                        };
                        if ui.add_enabled(!state.busy, egui::Button::new(label)).clicked() {
                            let tok = Zeroizing::new(std::mem::take(&mut self.ha_token_input));
                            clear_text_edit_undoer(ui.ctx(), egui::Id::unique(LOGIN_HA_TOK_EDIT_SALT));
                            self.send(Command::SaveHomeAssistantConfig {
                                data_source: "home_assistant".into(),
                                ha_url: self.ha_url_input.clone(),
                                ha_token: tok,
                                ha_sensor_entity_id: self.ha_entity_input.clone(),
                            });
                        }
                        if ui.button("Auto-Fill Env").clicked() {
                            self.send(Command::AutoFillHomeAssistantFromEnv);
                        }
                        if ui.button("Sample Data").clicked() {
                            self.send(Command::LoadDemo);
                        }
                    });
                }
            });
            ui.add_space(12.0);
            ui.label(RichText::new(&state.status).weak());
            ui.add_space(8.0);
            ui.label(
                RichText::new("Unofficial app — not affiliated with Abbott. Not a medical device; do not use for treatment decisions.")
                    .small()
                    .weak(),
            );
        });
    }

    fn submit_login(&mut self, ctx: &egui::Context) {
        let password = Zeroizing::new(std::mem::take(&mut self.login_password));
        clear_text_edit_undoer(ctx, egui::Id::unique(LOGIN_PW_EDIT_SALT));
        self.send(Command::Login {
            username: self.login_email.clone(),
            password,
            country: self.login_region.clone(),
        });
    }
}
