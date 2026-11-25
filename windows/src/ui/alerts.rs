//! Glucose threshold, audio alert, and snooze configuration tab.

use eframe::egui::{self, RichText};

use super::{LibreLinkUpApp, snooze_text};
use crate::service::{Command, Shared, now_epoch};

impl LibreLinkUpApp {
    pub(super) fn alerts_view(&mut self, ui: &mut egui::Ui, state: &Shared) {
        let mut s = state.settings.clone();
        if let Some(ref rx) = self.pending_sound_rx {
            match rx.try_recv() {
                Ok(Some(path)) => {
                    s.custom_sound_path = path.display().to_string();
                    self.pending_sound_rx = None;
                }
                Ok(None) => {
                    self.pending_sound_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pending_sound_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        ui.heading("Alerts");
        egui::Frame::group(ui.style())
            .inner_margin(12)
            .show(ui, |ui| {
                ui.checkbox(&mut s.audio_alert_enabled, "Play an alarm sound");
                ui.add_enabled_ui(s.audio_alert_enabled, |ui| {
                    ui.checkbox(
                        &mut s.use_custom_sound,
                        "Use a custom sound file (.wav / .mp3 / .ogg)",
                    );
                    if s.use_custom_sound {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut s.custom_sound_path)
                                    .desired_width(320.0),
                            );
                            let picking = self.pending_sound_rx.is_some();
                            if ui
                                .add_enabled(!picking, egui::Button::new("Browse…"))
                                .clicked()
                            {
                                let (tx, rx) = std::sync::mpsc::channel();
                                let ui_handle = self.ui_handle.clone();
                                self.pending_sound_rx = Some(rx);
                                let _ = std::thread::Builder::new()
                                    .name("llu-file-dialog".into())
                                    .spawn(move || {
                                        let picked = rfd::FileDialog::new()
                                            .add_filter("Audio", &["wav", "mp3", "ogg"])
                                            .pick_file();
                                        let _ = tx.send(picked);
                                        ui_handle.repaint_all();
                                    });
                            }
                        });
                    }
                });
                ui.checkbox(&mut s.flash_window_enabled, "Flash the taskbar button");
                ui.checkbox(
                    &mut s.bring_to_front_enabled,
                    "Bring the main window to the front",
                );
                ui.label(
                    RichText::new("A Windows notification is always shown.")
                        .small()
                        .weak(),
                );
            });
        ui.add_space(8.0);
        egui::Frame::group(ui.style())
            .inner_margin(12)
            .show(ui, |ui| {
                ui.label(RichText::new("Thresholds").strong());
                ui.checkbox(
                    &mut s.override_threshold,
                    "Override the sensor's target range",
                );
                egui::Grid::new("thresholds")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.add_enabled(s.override_threshold, egui::Label::new("Low alert below"));
                        ui.add_enabled(
                            s.override_threshold,
                            egui::DragValue::new(&mut s.custom_target_low)
                                .range(40.0..=180.0)
                                .suffix(" mg/dL"),
                        );
                        ui.end_row();
                        ui.add_enabled(s.override_threshold, egui::Label::new("High alert above"));
                        ui.add_enabled(
                            s.override_threshold,
                            egui::DragValue::new(&mut s.custom_target_high)
                                .range(100.0..=400.0)
                                .suffix(" mg/dL"),
                        );
                        ui.end_row();
                        ui.label("Urgent low below");
                        ui.add(
                            egui::DragValue::new(&mut s.urgent_low_mg_dl)
                                .range(40.0..=100.0)
                                .suffix(" mg/dL"),
                        );
                        ui.end_row();
                        ui.label("Repeat alerts every");
                        ui.add(
                            egui::DragValue::new(&mut s.alert_repeat_minutes)
                                .range(1..=240)
                                .suffix(" min"),
                        );
                        ui.end_row();
                        ui.checkbox(&mut s.alert_on_stale, "Alert when no new reading for");
                        ui.add(
                            egui::DragValue::new(&mut s.stale_after_minutes)
                                .range(5..=120)
                                .suffix(" min"),
                        );
                        ui.end_row();
                        ui.label("Sensor expiry warning (0 = off)");
                        ui.add(
                            egui::DragValue::new(&mut s.sensor_expiry_alert_hours)
                                .range(0..=168)
                                .suffix(" h"),
                        );
                        ui.end_row();
                    });
                ui.label(
                    RichText::new(
                        "Urgent low alerts repeat at least every 5 minutes and cannot be snoozed.",
                    )
                    .small()
                    .weak(),
                );
            });
        if s != state.settings {
            self.save_settings(s);
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let snooze = snooze_text(&state.settings);
            ui.label(if snooze.is_empty() {
                "Snooze alerts:".to_string()
            } else {
                snooze
            });
            if ui.button("30 min").clicked() {
                self.send(Command::Snooze(30));
            }
            if ui.button("1 hour").clicked() {
                self.send(Command::Snooze(60));
            }
            if ui.button("2 hours").clicked() {
                self.send(Command::Snooze(120));
            }
            if state.settings.is_snoozed(now_epoch()) && ui.button("Cancel snooze").clicked() {
                self.send(Command::Snooze(0));
            }
        });
        ui.add_space(8.0);
        if ui.button("▶ Test Alert Now").clicked() {
            self.send(Command::TestAlert);
        }
    }
}
