//! Authenticated CGM dashboard hero cards, glycemic stats, and 12h graph container.

use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, Stroke, Vec2};
use librelinkup_core::analytics::StatsWindow;
use librelinkup_core::models::{format_glucose_value, format_target_range, trend_arrow_description};

use super::{
    chart, darken_color, hex_color, paint_trend_arrow, reading_status, snooze_text,
    LibreLinkUpApp,
};
use crate::service::{now_epoch, Command, Shared};

impl LibreLinkUpApp {
    pub(super) fn dashboard(&mut self, ui: &mut egui::Ui, state: &Shared) {
        let s = &state.settings;
        let snap = &state.snapshot;
        let (label, color_hex, stale, age_text) = reading_status(state);
        let raw_color = hex_color(color_hex);
        let is_light_ui = !ui.visuals().dark_mode;
        let color = if is_light_ui {
            darken_color(raw_color, 1.42)
        } else {
            darken_color(raw_color, 1.15)
        };
        let hero_border = if is_light_ui {
            Stroke::new(2.0, darken_color(raw_color, 1.95))
        } else {
            Stroke::new(1.5, Color32::from_white_alpha(110))
        };
        let value = format_glucose_value(snap.value_mg_dl, &s.result_unit);

        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            // Hero card
            egui::Frame::new()
                .fill(color)
                .stroke(hero_border)
                .corner_radius(12)
                .inner_margin(16)
                .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(170.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("CURRENT GLUCOSE").small().strong().color(Color32::WHITE));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::Frame::new()
                            .fill(Color32::from_black_alpha(90))
                            .stroke(Stroke::new(1.0, Color32::from_white_alpha(115)))
                            .corner_radius(6)
                            .inner_margin(egui::Margin::symmetric(8, 2))
                            .show(ui, |ui| ui.label(RichText::new(label).small().strong().color(Color32::WHITE)));
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&value).size(52.0).strong().color(Color32::WHITE));
                    ui.vertical(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new(&s.result_unit).strong().color(Color32::WHITE));
                        ui.label(RichText::new(trend_arrow_description(snap.trend_arrow)).color(Color32::WHITE));
                    });
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
                    if snap.value_mg_dl > 0.0 {
                        paint_trend_arrow(ui.painter(), rect.center(), 40.0, snap.trend_arrow, Color32::WHITE);
                    }
                });
                let updated = if snap.timestamp.is_empty() {
                    "Updated: never".to_string()
                } else if age_text.is_empty() {
                    format!("Updated: {}", snap.timestamp)
                } else {
                    format!("Updated: {} ({age_text})", snap.timestamp)
                };
                let updated = if stale { format!("⚠ STALE — {updated}") } else { updated };
                ui.label(RichText::new(updated).small().color(Color32::WHITE));
                let snooze = snooze_text(s);
                if !snooze.is_empty() {
                    ui.label(RichText::new(format!("🔕 {snooze}")).small().color(Color32::WHITE));
                }
            });

            let ui = &mut cols[1];
            // Details card
            egui::Frame::group(ui.style()).inner_margin(14).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(170.0);
                ui.label(RichText::new("Details & Glycemic Analytics").strong());
                let low = s.effective_target_low(snap.target_low_mg_dl);
                let high = s.effective_target_high(snap.target_high_mg_dl);
                let now = now_epoch();
                let stats = self.compute_cached_stats(state, now);
                egui::Grid::new("details").num_columns(2).spacing([12.0, 5.0]).show(ui, |ui| {
                    ui.label("Target range");
                    ui.label(format_target_range(low, high, &s.result_unit));
                    ui.end_row();
                    ui.label("Estimated A1C");
                    ui.label(RichText::new(stats.format_headline()).strong())
                        .on_hover_text(stats.format_subline(&s.result_unit));
                    ui.end_row();
                    ui.label("Medications");
                    ui.label(state.medication_store.summary_line(now));
                    ui.end_row();
                    let name = format!("{} {}", snap.first_name, snap.last_name).trim().to_string();
                    ui.label("Patient");
                    if s.data_source == "home_assistant" && state.discovered_ha_patients.len() > 1 {
                        let cur_ent = s.ha_sensor_entity_id.trim();
                        let selected_label = state
                            .discovered_ha_patients
                            .iter()
                            .find(|p| p.entity_id.eq_ignore_ascii_case(cur_ent))
                            .map(|p| format!("{} ({})", p.patient_name, p.reading_display))
                            .unwrap_or_else(|| {
                                if name.is_empty() {
                                    "Auto-discover".to_string()
                                } else {
                                    format!("{name} (Auto)")
                                }
                            });
                        egui::ComboBox::from_id_salt("dashboard_ha_patient_combo")
                            .selected_text(selected_label)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(
                                        cur_ent.is_empty(),
                                        "Auto-discover (First Active Sensor)",
                                    )
                                    .clicked()
                                {
                                    self.ha_entity_input.clear();
                                    self.send(Command::SelectHaPatient(String::new()));
                                }
                                for p in &state.discovered_ha_patients {
                                    let is_sel = p.entity_id.eq_ignore_ascii_case(cur_ent);
                                    if ui.selectable_label(is_sel, &p.label).clicked() {
                                        self.ha_entity_input = p.entity_id.clone();
                                        self.send(Command::SelectHaPatient(p.entity_id.clone()));
                                    }
                                }
                            });
                    } else {
                        ui.label(if name.is_empty() { "—".to_string() } else { name });
                    }
                    ui.end_row();
                    ui.label("Sensor");
                    let sensor_txt = snap.format_sensor_status(now);
                    let sensor_warn = snap.is_sensor_in_warmup(now)
                        || snap.is_sensor_expiring_soon(now, s.sensor_expiry_alert_hours);
                    if sensor_warn {
                        ui.label(RichText::new(sensor_txt).strong().color(Color32::from_rgb(234, 179, 8)));
                    } else {
                        ui.label(sensor_txt);
                    }
                    ui.end_row();
                    ui.label("Region");
                    ui.label(s.country.to_uppercase());
                    ui.end_row();
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Stats window:").small());
                    for w in StatsWindow::ALL {
                        if ui
                            .selectable_label(state.stats_window == w, w.code())
                            .clicked()
                        {
                            self.send(Command::SetStatsWindow(w));
                        }
                    }
                    let mut overlay = s.overlay_visible;
                    if ui.checkbox(&mut overlay, "Overlay").changed() {
                        let mut ns = s.clone();
                        ns.overlay_visible = overlay;
                        self.save_settings(ns);
                    }
                });
            });
        });

        ui.add_space(10.0);
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.label(RichText::new("Last 12 hours").strong());
            let now = now_epoch();
            let points = self.cached_graph_points_for(state, now).to_vec();
            chart::glucose_graph(ui, state, &points, now);
        });
    }
}
