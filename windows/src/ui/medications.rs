//! Medication schedule, due reminders, and 90-day dose history tab.

use eframe::egui::{self, Align, Layout, RichText};
use librelinkup_core::analytics::StatsWindow;
use librelinkup_core::medications::{MedicationSchedule, weekday_full_name};

use super::LibreLinkUpApp;
use crate::service::{Command, Shared, now_epoch};

impl LibreLinkUpApp {
    pub(super) fn medications_view(&mut self, ui: &mut egui::Ui, state: &Shared) {
        let s = &state.settings;
        let now = now_epoch();
        let stats = self.compute_cached_stats(state, now);

        ui.heading("Medications, Dose Log & Estimated A1C");

        // Glycemic Analytics Card
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Estimated A1C (eA1C) & GMI").strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for w in StatsWindow::ALL.iter().rev() {
                        if ui
                            .selectable_label(state.stats_window == *w, w.code())
                            .clicked()
                        {
                            self.send(Command::SetStatsWindow(*w));
                        }
                    }
                });
            });
            ui.label(RichText::new(stats.format_headline()).size(15.0).strong());
            ui.label(stats.format_subline(&s.result_unit));
            ui.label(
                RichText::new(
                    "Formulas: ADAG eA1C (%) = (Mean mg/dL + 46.7) / 28.7 • Bergenstal GMI (%) = 3.31 + 0.02392 × Mean mg/dL.",
                )
                .small()
                .weak(),
            );
        });

        ui.add_space(8.0);

        // Scheduled Medications & Quick Dose Checklist
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Scheduled Medications & Quick Dose Checklist").strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(state.medication_store.summary_line(now)).strong());
                });
            });
            let items = state.medication_store.view_items(now);
            if items.is_empty() {
                ui.label(
                    RichText::new(
                        "No medications configured yet. Add your oral medications or insulin below.",
                    )
                    .weak(),
                );
            } else {
                for item in items {
                    egui::Frame::new()
                        .fill(ui.visuals().faint_bg_color)
                        .corner_radius(6)
                        .inner_margin(8)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(format!("{} — {}", item.name, item.dosage))
                                            .strong(),
                                    );
                                    let sched_info = if item.schedule_code == "as_needed" {
                                        format!("{} • {}", item.schedule_label, item.status_label)
                                    } else if item.schedule_code == "weekly" {
                                        format!(
                                            "{} ({} {}) • {}",
                                            item.schedule_label,
                                            item.weekday_label,
                                            item.scheduled_time,
                                            item.status_label
                                        )
                                    } else {
                                        format!(
                                            "{} ({}) • {}",
                                            item.schedule_label, item.scheduled_time, item.status_label
                                        )
                                    };
                                    ui.label(RichText::new(sched_info).small());
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.button("🗑").on_hover_text("Remove medication").clicked() {
                                        if self.med_editing_id == item.id {
                                            self.med_editing_id.clear();
                                        }
                                        self.send(Command::RemoveMedication(item.id.clone()));
                                    }
                                    if ui
                                        .button("✏")
                                        .on_hover_text("Edit medication dosage or schedule")
                                        .clicked()
                                    {
                                        self.med_editing_id = item.id.clone();
                                        self.med_name = item.name.clone();
                                        self.med_dosage = item.dosage.clone();
                                        self.med_schedule =
                                            MedicationSchedule::from_code(&item.schedule_code);
                                        self.med_weekday = item.weekday;
                                        self.med_times = item.scheduled_time.clone();
                                        self.med_reminder = item.reminder_enabled;
                                    }
                                    if item.schedule_code != "as_needed"
                                        && ui.button("⏭ Skipped").clicked()
                                    {
                                        self.send(Command::SkipMedicationDose {
                                            medication_id: item.id.clone(),
                                            notes: String::new(),
                                        });
                                    }
                                    if ui.button("✔ Taken").clicked() {
                                        self.send(Command::LogMedicationDose {
                                            medication_id: item.id.clone(),
                                            custom_dosage: String::new(),
                                            notes: String::new(),
                                        });
                                    }
                                });
                            });
                        });
                    ui.add_space(4.0);
                }
            }
        });

        ui.add_space(8.0);

        // Add / Edit Medication Form
        egui::Frame::group(ui.style())
            .inner_margin(12)
            .show(ui, |ui| {
                let is_editing = !self.med_editing_id.is_empty();
                ui.label(
                    RichText::new(if is_editing {
                        "Edit Medication Dosage or Schedule"
                    } else {
                        "Add Scheduled or As-Needed Medication"
                    })
                    .strong(),
                );
                egui::Grid::new("add_med_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Medication name");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.med_name)
                                .hint_text("e.g. Metformin ER, Lantus, Lispro")
                                .desired_width(240.0),
                        );
                        ui.end_row();
                        ui.label("Dosage");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.med_dosage)
                                .hint_text("e.g. 500 mg, 18 units")
                                .desired_width(240.0),
                        );
                        ui.end_row();
                        ui.label("Schedule");
                        ui.horizontal(|ui| {
                            for sched in [
                                MedicationSchedule::Daily,
                                MedicationSchedule::TwiceDaily,
                                MedicationSchedule::Weekly,
                                MedicationSchedule::AsNeeded,
                            ] {
                                if ui
                                    .selectable_value(&mut self.med_schedule, sched, sched.label())
                                    .clicked()
                                {
                                    if sched == MedicationSchedule::TwiceDaily
                                        && self.med_times == "08:00"
                                    {
                                        self.med_times = "08:00, 20:00".to_string();
                                    } else if sched == MedicationSchedule::Daily
                                        && self.med_times == "08:00, 20:00"
                                    {
                                        self.med_times = "08:00".to_string();
                                    }
                                }
                            }
                        });
                        ui.end_row();
                        if self.med_schedule == MedicationSchedule::Weekly {
                            ui.label("Day of week");
                            egui::ComboBox::from_id_salt("med_weekday_combo")
                                .selected_text(weekday_full_name(self.med_weekday))
                                .show_ui(ui, |ui| {
                                    for wd in 0..7u8 {
                                        ui.selectable_value(
                                            &mut self.med_weekday,
                                            wd,
                                            weekday_full_name(wd),
                                        );
                                    }
                                });
                            ui.end_row();
                        }
                        if self.med_schedule != MedicationSchedule::AsNeeded {
                            ui.label("Scheduled time(s)");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.med_times)
                                    .hint_text("08:00 or 08:00, 20:00")
                                    .desired_width(240.0),
                            );
                            ui.end_row();
                        }
                    });
                ui.horizontal(|ui| {
                    ui.checkbox(
                        &mut self.med_reminder,
                        "Desktop reminder notification when due",
                    );
                    if is_editing && ui.button("✖ Cancel").clicked() {
                        self.med_editing_id.clear();
                        self.med_name.clear();
                        self.med_dosage.clear();
                        self.med_schedule = MedicationSchedule::Daily;
                        self.med_weekday = 0;
                        self.med_times = "08:00".to_string();
                        self.med_reminder = true;
                    }
                    let save_label = if is_editing {
                        "💾 Save Changes"
                    } else {
                        "➕ Add Medication"
                    };
                    if ui
                        .add_enabled(
                            !self.med_name.trim().is_empty(),
                            egui::Button::new(save_label),
                        )
                        .clicked()
                    {
                        let id = std::mem::take(&mut self.med_editing_id);
                        let name = std::mem::take(&mut self.med_name);
                        let dosage = std::mem::take(&mut self.med_dosage);
                        self.send(Command::UpsertMedication {
                            id,
                            name,
                            dosage,
                            schedule: self.med_schedule,
                            scheduled_time: self.med_times.clone(),
                            weekday: self.med_weekday,
                            reminder_enabled: self.med_reminder,
                            notes: String::new(),
                        });
                    }
                });
            });

        ui.add_space(8.0);

        // Recent Dose History
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "Recent Dose History ({} logged)",
                    state.medication_store.dose_logs.len()
                ))
                .strong(),
            );
            if state.medication_store.dose_logs.is_empty() {
                ui.label(
                    RichText::new(
                        "No doses logged yet. Doses within the last 12 hours are also plotted as purple markers (💊) on the Dashboard glucose trend graph.",
                    )
                    .weak(),
                );
            } else {
                let total_rows = state.medication_store.dose_logs.len().min(200);
                egui::ScrollArea::vertical()
                    .id_salt("dose_history_scroll")
                    .max_height(260.0)
                    .show_rows(ui, 24.0, total_rows, |ui, row_range| {
                        for idx in row_range {
                            let dose = &state.medication_store.dose_logs[idx];
                            ui.horizontal(|ui| {
                                let label_txt = if dose.skipped {
                                    format!("⏭ {} ({}) — Skipped", dose.medication_name, dose.dosage)
                                } else {
                                    format!("💊 {} ({})", dose.medication_name, dose.dosage)
                                };
                                ui.label(RichText::new(label_txt).strong());
                                let g_part = dose
                                    .glucose_mg_dl_at_dose
                                    .map(|g| format!(" • {:.0} mg/dL", g))
                                    .unwrap_or_default();
                                ui.label(
                                    RichText::new(format!("{}{}", dose.taken_at_display, g_part)).small(),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui
                                        .button("🗑")
                                        .on_hover_text("Remove dose log entry")
                                        .clicked()
                                    {
                                        self.send(Command::RemoveMedicationDose(dose.id.clone()));
                                    }
                                });
                            });
                        }
                    });
            }
        });
    }
}
