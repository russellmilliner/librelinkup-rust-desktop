//! Diagnostic log viewer (virtualized) and application About/updates view.

use eframe::egui::{self, Color32, RichText};
use librelinkup_core::event_log::{clear_events, export_events_to_path};

use super::LibreLinkUpApp;
use crate::service::{Command, Shared};

impl LibreLinkUpApp {
    pub(super) fn logs_view(&mut self, ui: &mut egui::Ui) {
        if let Some(ref rx) = self.pending_export_rx {
            match rx.try_recv() {
                Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pending_export_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        ui.heading("Diagnostic Log");
        let levels = ["ALL", "INFO", "WARN", "ERROR"];
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.log_filter).hint_text("Search…").desired_width(220.0));
            egui::ComboBox::from_id_salt("log-level")
                .selected_text(levels[self.log_min_level])
                .show_ui(ui, |ui| {
                    for (i, l) in levels.iter().enumerate() {
                        ui.selectable_value(&mut self.log_min_level, i, *l);
                    }
                });
            if ui.button("Clear").clicked() {
                clear_events();
            }
            let exporting = self.pending_export_rx.is_some();
            if ui.add_enabled(!exporting, egui::Button::new("Export…")).clicked() {
                let (tx, rx) = std::sync::mpsc::channel();
                let ui_handle = self.ui_handle.clone();
                self.pending_export_rx = Some(rx);
                let _ = std::thread::Builder::new()
                    .name("llu-export-dialog".into())
                    .spawn(move || {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("librelinkup-windows.log")
                            .add_filter("Log", &["log", "txt"])
                            .save_file()
                        {
                            let _ = export_events_to_path(&path);
                        }
                        let _ = tx.send(());
                        ui_handle.repaint_all();
                    });
            }
        });
        ui.separator();
        self.ensure_cached_logs();
        let row_count = self.cached_log_rows.len();
        if row_count == 0 {
            ui.label(RichText::new("No matching log entries.").weak());
            return;
        }
        let normal_color = ui.visuals().text_color();
        let warn_color = Color32::from_rgb(234, 179, 8);
        let err_color = Color32::from_rgb(239, 68, 68);
        egui::ScrollArea::vertical()
            .id_salt("logs_virtual_scroll")
            .auto_shrink([false, false])
            .show_rows(ui, 18.0, row_count, |ui, row_range| {
                for idx in row_range {
                    let (tag, line) = &self.cached_log_rows[idx];
                    let color = match *tag {
                        2 => err_color,
                        1 => warn_color,
                        _ => normal_color,
                    };
                    ui.label(RichText::new(line).monospace().size(11.5).color(color));
                }
            });
    }

    pub(super) fn about_view(&mut self, ui: &mut egui::Ui, state: &Shared) {
        ui.heading("LibreLinkUp Rust Desktop for Windows");
        ui.label(format!("Version {} • Native Rust + egui/eframe (Windows 10/11 x64)", env!("CARGO_PKG_VERSION")));
        ui.add_space(8.0);
        ui.label(
            "Continuous blood glucose monitor for LibreLinkUp featuring Windows DPAPI + AES-256-GCM \
             encrypted credentials, 90-day glycemic analytics (eA1C / GMI / TIR%), medication tracking, \
             system tray glucose badge & tooltip, interactive 12-hour CGM chart, \
             stale-reading & urgent-low/snooze alert engine, and an always-on-top desktop overlay \
             with scroll-wheel transparency and quick action buttons. Contains zero tracking and zero advertisements — forever.",
        );
        ui.add_space(8.0);
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.horizontal(|ui| {
                let btn_label = if state.checking_updates {
                    "Checking..."
                } else {
                    "🔄 Check for Updates"
                };
                if ui
                    .add_enabled(!state.checking_updates, egui::Button::new(btn_label))
                    .clicked()
                {
                    self.send(Command::CheckUpdates { manual: true });
                }
                if let Some(ref upd) = state.update_info {
                    ui.label(&upd.message);
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
                        ui.hyperlink_to("View Release", &upd.release_url);
                    }
                } else {
                    ui.label(format!("Installed: v{}", env!("CARGO_PKG_VERSION")));
                }
            });
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Unofficial app — not affiliated with or endorsed by Abbott. LibreLinkUp is a trademark of Abbott.").weak());
        ui.label(RichText::new("Not a medical device. Do not use for treatment decisions.").weak());
        ui.add_space(8.0);
        ui.hyperlink_to("GitHub: russellmilliner/librelinkup-rust-desktop", "https://github.com/russellmilliner/librelinkup-rust-desktop");
        ui.hyperlink_to("Windows Documentation (windows/README.md)", "https://github.com/russellmilliner/librelinkup-rust-desktop/blob/main/windows/README.md");
        ui.hyperlink_to("Based on Crazy-Marvin/LibreLinkUpDesktop (Apache-2.0)", "https://github.com/Crazy-Marvin/LibreLinkUpDesktop");
        ui.hyperlink_to("☕ Support on Ko-fi", "https://ko-fi.com/russellmilliner");
        ui.add_space(8.0);
        ui.label("Licensed under the Apache License 2.0.");
    }
}
