//! Main window (egui) and floating overlay viewport.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Pos2, Rect, RichText, Sense, Shape,
    Stroke, Vec2, ViewportBuilder, ViewportCommand, ViewportId,
};
use librelinkup_core::api::normalize_country_code;
use librelinkup_core::event_log::{clear_events, export_events_to_path, get_all_events};
use librelinkup_core::models::{
    convert_mg_dl_to_unit, format_glucose_value, format_reading_age, format_target_range,
    parse_libreview_timestamp, reading_age_minutes, trend_arrow_description, trend_arrow_symbol,
    AppSettings, GlucoseRangeStatus, MAX_POLL_INTERVAL_SECONDS, MIN_POLL_INTERVAL_SECONDS,
    STALE_COLOR_HEX,
};
use librelinkup_core::paths::credentials_file_path;
use librelinkup_core::session::format_snooze_until;
use tokio::sync::mpsc::UnboundedSender;

use crate::service::{now_epoch, Command, Shared, SharedState};
use crate::tray::TrayView;

pub const OVERLAY_ID: &str = "llu-overlay";

pub const REGIONS: &[(&str, &str)] = &[
    ("us", "United States"),
    ("global", "Global"),
    ("eu", "European Union"),
    ("eu2", "European Union 2 / UK"),
    ("de", "Germany"),
    ("fr", "France"),
    ("ca", "Canada"),
    ("au", "Australia"),
    ("ap", "Asia / Pacific"),
    ("jp", "Japan"),
    ("ae", "United Arab Emirates"),
    ("la", "Latin America"),
];

/// Cross-thread handle to the main window (used by the tray and the service notifier).
pub struct UiHandle {
    pub ctx: egui::Context,
    pub hwnd: AtomicIsize,
    pub window_visible: AtomicBool,
    pub quitting: AtomicBool,
}

impl UiHandle {
    pub fn show_main_window(&self) {
        self.window_visible.store(true, Ordering::SeqCst);
        self.ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(true));
        self.ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus);
        crate::platform::show_window(self.hwnd.load(Ordering::SeqCst), true);
        self.ctx.request_repaint();
    }

    pub fn hide_main_window(&self) {
        self.window_visible.store(false, Ordering::SeqCst);
        self.ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(false));
        self.ctx.request_repaint();
    }

    pub fn toggle_main_window(&self) {
        if self.window_visible.load(Ordering::SeqCst) {
            self.hide_main_window();
        } else {
            self.show_main_window();
        }
    }

    pub fn quit(&self) {
        self.quitting.store(true, Ordering::SeqCst);
        self.ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(true));
        self.ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        self.ctx.request_repaint();
    }

    pub fn repaint_all(&self) {
        self.ctx.request_repaint();
        self.ctx.request_repaint_of(ViewportId::from_hash_of(OVERLAY_ID));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dashboard,
    Settings,
    Alerts,
    Account,
    Logs,
    About,
}

pub struct LibreLinkUpApp {
    shared: SharedState,
    tx: UnboundedSender<Command>,
    ui_handle: Arc<UiHandle>,
    tab: Tab,
    login_email: String,
    login_password: String,
    login_region: String,
    log_filter: String,
    log_min_level: usize,
}

impl LibreLinkUpApp {
    pub fn new(shared: SharedState, tx: UnboundedSender<Command>, ui_handle: Arc<UiHandle>) -> Self {
        let region = shared
            .lock()
            .map(|s| normalize_country_code(&s.settings.country))
            .unwrap_or_else(|_| "us".into());
        Self {
            shared,
            tx,
            ui_handle,
            tab: Tab::Dashboard,
            login_email: String::new(),
            login_password: String::new(),
            login_region: if region.is_empty() { "us".into() } else { region },
            log_filter: String::new(),
            log_min_level: 0,
        }
    }

    fn snapshot(&self) -> Shared {
        self.shared.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    fn save_settings(&self, settings: AppSettings) {
        if let Ok(mut s) = self.shared.lock() {
            s.settings = settings.clone();
        }
        self.send(Command::SaveSettings(Box::new(settings)));
        self.ui_handle.repaint_all();
    }
}

// ----------------------------------------------------------------------------------------
// Presentation helpers shared by the window, overlay and tray
// ----------------------------------------------------------------------------------------

pub fn hex_color(hex: &str) -> Color32 {
    let h = hex.trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0x22c55e);
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// (label, color hex, stale?, age text) for the current reading.
pub fn reading_status(state: &Shared) -> (&'static str, &'static str, bool, String) {
    let s = &state.settings;
    let snap = &state.snapshot;
    let age = reading_age_minutes(snap.measured_at_epoch, now_epoch());
    let age_text = format_reading_age(age);
    if snap.value_mg_dl <= 0.0 {
        return ("Waiting", "#22c55e", false, age_text);
    }
    if s.is_reading_stale(age) {
        return ("Stale", STALE_COLOR_HEX, true, age_text);
    }
    let low = s.effective_target_low(snap.target_low_mg_dl);
    let high = s.effective_target_high(snap.target_high_mg_dl);
    let st = GlucoseRangeStatus::classify(snap.value_mg_dl, low, high);
    (st.label(), st.color_hex(), false, age_text)
}

pub fn snooze_text(settings: &AppSettings) -> String {
    format_snooze_until(settings.snooze_until_epoch, now_epoch())
}

pub fn build_tray_view(state: &Shared) -> TrayView {
    let s = &state.settings;
    let snap = &state.snapshot;
    let (label, color, stale, age_text) = reading_status(state);
    let value = format_glucose_value(snap.value_mg_dl, &s.result_unit);
    let c = hex_color(color);
    let badge = if s.show_glucose_in_tray_icon && snap.value_mg_dl > 0.0 { value.clone() } else { String::new() };
    let mut tooltip = if !s.show_glucose_in_tray_tooltip {
        "LibreLinkUp Rust Desktop".to_string()
    } else if snap.value_mg_dl <= 0.0 {
        "LibreLinkUp — waiting for glucose reading".to_string()
    } else {
        format!(
            "{value} {} {} ({label})\n{}",
            s.result_unit,
            trend_arrow_symbol(snap.trend_arrow),
            if age_text.is_empty() { snap.timestamp.clone() } else { age_text.clone() }
        )
    };
    if stale && s.show_glucose_in_tray_tooltip {
        tooltip = format!("STALE — {tooltip}");
    }
    let status_line = if snap.value_mg_dl > 0.0 {
        format!(
            "Blood Sugar: {value} {} {}{}",
            s.result_unit,
            trend_arrow_symbol(snap.trend_arrow),
            if stale { " (STALE)" } else { "" }
        )
    } else {
        "Blood Sugar: --".to_string()
    };
    TrayView {
        badge,
        badge_color: [c.r(), c.g(), c.b()],
        tooltip,
        status_line,
        overlay_visible: s.overlay_visible,
        autostart: s.autostart_enabled,
        snooze_line: snooze_text(s),
    }
}

// ----------------------------------------------------------------------------------------
// eframe App
// ----------------------------------------------------------------------------------------

impl eframe::App for LibreLinkUpApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Closing the main window hides it to the tray; Quit (tray menu) really exits.
        if ctx.input(|i| i.viewport().close_requested()) && !self.ui_handle.quitting.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.ui_handle.hide_main_window();
        }
        // Keep the overlay viewport alive and updating while the main window is hidden.
        let overlay = self.shared.lock().map(|s| s.settings.overlay_visible).unwrap_or(false);
        if overlay && !self.ui_handle.window_visible.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(ViewportCommand::RequestPaintWhileHidden);
        }
        // Reading age / snooze countdowns change over time even without new data.
        ctx.request_repaint_after(std::time::Duration::from_secs(30));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // lets the overlay viewport be transparent
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let state = self.snapshot();
        apply_theme(ui.ctx(), &state.settings.theme_mode);

        egui::Panel::top("header").show(ui, |ui| self.header(ui, &state));

        egui::CentralPanel::default_margins().show(ui, |ui| {
            if !state.error.is_empty() {
                egui::Frame::new()
                    .fill(Color32::from_rgb(127, 29, 29))
                    .corner_radius(6)
                    .inner_margin(8)
                    .show(ui, |ui| {
                        ui.label(RichText::new(format!("⚠ {}", state.error)).color(Color32::WHITE));
                    });
                ui.add_space(6.0);
            }
            if let Some(ref upd) = state.update_info {
                if upd.update_available {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(3, 105, 161))
                        .corner_radius(6)
                        .inner_margin(8)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "⬆ New version available: {} (current: {})",
                                        upd.latest_version, upd.current_version
                                    ))
                                    .strong()
                                    .color(Color32::WHITE),
                                );
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
                                ui.hyperlink_to(
                                    RichText::new("View Release Notes").color(Color32::from_rgb(224, 242, 254)).underline(),
                                    &upd.release_url,
                                );
                            });
                        });
                    ui.add_space(6.0);
                }
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                Tab::Dashboard if !state.logged_in => self.login_view(ui, &state),
                Tab::Dashboard => self.dashboard(ui, &state),
                Tab::Settings => self.settings_view(ui, &state),
                Tab::Alerts => self.alerts_view(ui, &state),
                Tab::Account => self.account_view(ui, &state),
                Tab::Logs => self.logs_view(ui),
                Tab::About => self.about_view(ui, &state),
            });
        });

        if state.settings.overlay_visible {
            self.show_overlay(ui.ctx());
        }
    }
}

fn apply_theme(ctx: &egui::Context, mode: &str) {
    let pref = match mode {
        "dark" => egui::ThemePreference::Dark,
        "light" => egui::ThemePreference::Light,
        _ => egui::ThemePreference::System,
    };
    ctx.set_theme(pref);
}

impl LibreLinkUpApp {
    fn header(&mut self, ui: &mut egui::Ui, state: &Shared) {
        ui.horizontal_wrapped(|ui| {
            ui.heading(RichText::new("LibreLinkUp").strong());
            let (badge, color) = if state.demo {
                ("Sample data", Color32::from_rgb(234, 179, 8))
            } else if state.vault_ok {
                ("🔒 DPAPI", Color32::from_rgb(34, 197, 94))
            } else {
                ("🔓 Not signed in", Color32::GRAY)
            };
            ui.label(RichText::new(badge).small().color(color));
            ui.separator();
            for (tab, label) in [
                (Tab::Dashboard, "📈 Dashboard"),
                (Tab::Settings, "⚙ Settings"),
                (Tab::Alerts, "🔔 Alerts"),
                (Tab::Account, "👤 Account"),
                (Tab::Logs, "📜 Logs"),
                (Tab::About, "ℹ About"),
            ] {
                ui.selectable_value(&mut self.tab, tab, label);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("🗕").on_hover_text("Hide to the system tray").clicked() {
                    self.ui_handle.hide_main_window();
                }
                let refresh = ui.add_enabled(state.logged_in && !state.busy && !state.demo, egui::Button::new("🔄"));
                if refresh.on_hover_text("Refresh glucose now").clicked() {
                    self.send(Command::RefreshNow);
                }
                if state.busy {
                    ui.spinner();
                }
            });
        });
    }

    fn login_view(&mut self, ui: &mut egui::Ui, state: &Shared) {
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);
            ui.heading("Sign in to LibreLinkUp");
            ui.label("Use the email and password of your LibreLinkUp follower account.");
            ui.add_space(12.0);
            egui::Frame::group(ui.style()).inner_margin(16).show(ui, |ui| {
                ui.set_max_width(420.0);
                egui::Grid::new("login").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                    ui.label("Email");
                    ui.add(egui::TextEdit::singleline(&mut self.login_email).hint_text("you@example.com").desired_width(260.0));
                    ui.end_row();
                    ui.label("Password");
                    let pw = ui.add(egui::TextEdit::singleline(&mut self.login_password).password(true).desired_width(260.0));
                    ui.end_row();
                    ui.label("Region");
                    region_combo(ui, &mut self.login_region);
                    ui.end_row();
                    if pw.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.submit_login();
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
                        self.submit_login();
                    }
                    if ui.button("Sample Data").clicked() {
                        self.send(Command::LoadDemo);
                    }
                });
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

    fn submit_login(&mut self) {
        let password = std::mem::take(&mut self.login_password);
        self.send(Command::Login {
            username: self.login_email.clone(),
            password,
            country: self.login_region.clone(),
        });
    }

    fn dashboard(&mut self, ui: &mut egui::Ui, state: &Shared) {
        let s = &state.settings;
        let snap = &state.snapshot;
        let (label, color_hex, stale, age_text) = reading_status(state);
        let color = hex_color(color_hex);
        let value = format_glucose_value(snap.value_mg_dl, &s.result_unit);

        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            // Hero card
            egui::Frame::new().fill(color).corner_radius(12).inner_margin(16).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(170.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("CURRENT GLUCOSE").small().strong().color(Color32::WHITE));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::Frame::new()
                            .fill(Color32::from_black_alpha(60))
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
                ui.label(RichText::new("Details").strong());
                let low = s.effective_target_low(snap.target_low_mg_dl);
                let high = s.effective_target_high(snap.target_high_mg_dl);
                egui::Grid::new("details").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("Target range");
                    ui.label(format_target_range(low, high, &s.result_unit));
                    ui.end_row();
                    let name = format!("{} {}", snap.first_name, snap.last_name).trim().to_string();
                    ui.label("Patient");
                    ui.label(if name.is_empty() { "—".to_string() } else { name });
                    ui.end_row();
                    ui.label("Region");
                    ui.label(s.country.to_uppercase());
                    ui.end_row();
                    ui.label("Status");
                    ui.label(&state.status);
                    ui.end_row();
                });
                ui.add_space(6.0);
                let mut overlay = s.overlay_visible;
                if ui.checkbox(&mut overlay, "Floating desktop overlay").changed() {
                    let mut ns = s.clone();
                    ns.overlay_visible = overlay;
                    self.save_settings(ns);
                }
            });
        });

        ui.add_space(10.0);
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.label(RichText::new("Last 12 hours").strong());
            glucose_graph(ui, state);
        });
    }

    fn settings_view(&mut self, ui: &mut egui::Ui, state: &Shared) {
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
        if s != state.settings {
            self.save_settings(s);
        }
    }

    fn alerts_view(&mut self, ui: &mut egui::Ui, state: &Shared) {
        let mut s = state.settings.clone();
        ui.heading("Alerts");
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.checkbox(&mut s.audio_alert_enabled, "Play an alarm sound");
            ui.add_enabled_ui(s.audio_alert_enabled, |ui| {
                ui.checkbox(&mut s.use_custom_sound, "Use a custom sound file (.wav / .mp3 / .ogg)");
                if s.use_custom_sound {
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut s.custom_sound_path).desired_width(320.0));
                        if ui.button("Browse…").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Audio", &["wav", "mp3", "ogg"])
                                .pick_file()
                            {
                                s.custom_sound_path = path.display().to_string();
                            }
                        }
                    });
                }
            });
            ui.checkbox(&mut s.flash_window_enabled, "Flash the taskbar button");
            ui.checkbox(&mut s.bring_to_front_enabled, "Bring the main window to the front");
            ui.label(RichText::new("A Windows notification is always shown.").small().weak());
        });
        ui.add_space(8.0);
        egui::Frame::group(ui.style()).inner_margin(12).show(ui, |ui| {
            ui.label(RichText::new("Thresholds").strong());
            ui.checkbox(&mut s.override_threshold, "Override the sensor's target range");
            egui::Grid::new("thresholds").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                ui.add_enabled(s.override_threshold, egui::Label::new("Low alert below"));
                ui.add_enabled(s.override_threshold, egui::DragValue::new(&mut s.custom_target_low).range(40.0..=180.0).suffix(" mg/dL"));
                ui.end_row();
                ui.add_enabled(s.override_threshold, egui::Label::new("High alert above"));
                ui.add_enabled(s.override_threshold, egui::DragValue::new(&mut s.custom_target_high).range(100.0..=400.0).suffix(" mg/dL"));
                ui.end_row();
                ui.label("Urgent low below");
                ui.add(egui::DragValue::new(&mut s.urgent_low_mg_dl).range(40.0..=100.0).suffix(" mg/dL"));
                ui.end_row();
                ui.label("Repeat alerts every");
                ui.add(egui::DragValue::new(&mut s.alert_repeat_minutes).range(1..=240).suffix(" min"));
                ui.end_row();
                ui.checkbox(&mut s.alert_on_stale, "Alert when no new reading for");
                ui.add(egui::DragValue::new(&mut s.stale_after_minutes).range(5..=120).suffix(" min"));
                ui.end_row();
            });
            ui.label(RichText::new("Urgent low alerts repeat at least every 5 minutes and cannot be snoozed.").small().weak());
        });
        if s != state.settings {
            self.save_settings(s);
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let snooze = snooze_text(&state.settings);
            ui.label(if snooze.is_empty() { "Snooze alerts:".to_string() } else { snooze });
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

    fn account_view(&mut self, ui: &mut egui::Ui, state: &Shared) {
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
                self.send(Command::Logout);
                self.tab = Tab::Dashboard;
            }
        });
    }

    fn logs_view(&mut self, ui: &mut egui::Ui) {
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
            if ui.button("Export…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("librelinkup-windows.log")
                    .add_filter("Log", &["log", "txt"])
                    .save_file()
                {
                    let _ = export_events_to_path(&path);
                }
            }
        });
        ui.separator();
        let filter = self.log_filter.to_lowercase();
        let rank = |l: &str| match l {
            "ERROR" => 3,
            "WARN" => 2,
            "INFO" => 1,
            _ => 0,
        };
        let min = self.log_min_level;
        for e in get_all_events().iter().rev() {
            if rank(&e.level) < min {
                continue;
            }
            let line = format!("{}  [{}] {}: {}", e.timestamp, e.level, e.category, e.message);
            if !filter.is_empty() && !line.to_lowercase().contains(&filter) {
                continue;
            }
            let color = match e.level.as_str() {
                "ERROR" => Color32::from_rgb(239, 68, 68),
                "WARN" => Color32::from_rgb(234, 179, 8),
                _ => ui.visuals().text_color(),
            };
            ui.label(RichText::new(line).monospace().size(11.5).color(color));
        }
    }

    fn show_overlay(&self, ctx: &egui::Context) {
        let (settings, _) = {
            let s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
            (s.settings.clone(), ())
        };
        let mut builder = ViewportBuilder::default()
            .with_title("LibreLinkUp Overlay")
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_inner_size([settings.overlay_width.clamp(200, 600) as f32, settings.overlay_height.clamp(70, 260) as f32]);
        if settings.overlay_x >= 0 && settings.overlay_y >= 0 {
            builder = builder.with_position([settings.overlay_x as f32, settings.overlay_y as f32]);
        }
        let shared = self.shared.clone();
        let tx = self.tx.clone();
        let ui_handle = self.ui_handle.clone();
        ctx.show_viewport_deferred(ViewportId::from_hash_of(OVERLAY_ID), builder, move |ui, _class| {
            overlay_ui(ui, &shared, &tx, &ui_handle);
        });
    }
}

/// Draws the LibreLinkUp trend arrow (1 = falling quickly … 5 = rising quickly) as a vector
/// shape, so it renders identically regardless of installed fonts.
pub fn paint_trend_arrow(painter: &egui::Painter, center: Pos2, size: f32, trend: i32, color: Color32) {
    let angle_deg: f32 = match trend {
        1 => 90.0,
        2 => 45.0,
        4 => -45.0,
        5 => -90.0,
        _ => 0.0,
    };
    let dir = Vec2::angled(angle_deg.to_radians());
    let normal = Vec2::new(-dir.y, dir.x);
    let half = size * 0.45;
    let tail = center - dir * half;
    let tip = center + dir * half;
    let head_len = size * 0.38;
    let head_base = tip - dir * head_len;
    let stroke = Stroke::new((size * 0.13).max(2.0), color);
    painter.line_segment([tail, head_base + dir * 1.0], stroke);
    painter.add(Shape::convex_polygon(
        vec![tip, head_base + normal * head_len * 0.6, head_base - normal * head_len * 0.6],
        color,
        Stroke::NONE,
    ));
}

fn region_combo(ui: &mut egui::Ui, region: &mut String) {
    let current = REGIONS.iter().find(|(c, _)| c == region).map(|(_, n)| *n).unwrap_or("United States");
    egui::ComboBox::from_id_salt("region")
        .selected_text(format!("{current} ({region})"))
        .width(260.0)
        .show_ui(ui, |ui| {
            for (code, name) in REGIONS {
                ui.selectable_value(region, code.to_string(), format!("{name} ({code})"));
            }
        });
}

/// 12-hour trend graph with target band and hover readout.
fn glucose_graph(ui: &mut egui::Ui, state: &Shared) {
    let s = &state.settings;
    let snap = &state.snapshot;
    let width = ui.available_width().max(300.0);
    let (response, painter) = ui.allocate_painter(Vec2::new(width, 220.0), Sense::hover());
    let rect = response.rect;
    let visuals = ui.visuals().clone();
    painter.rect_filled(rect, CornerRadius::same(6), visuals.extreme_bg_color);

    let now = now_epoch();
    let start = now - 12 * 3600;
    let mut points: Vec<(i64, f64)> = snap
        .graph_points
        .iter()
        .filter_map(|p| parse_libreview_timestamp(&p.timestamp, false).map(|t| (t, p.value_mg_dl)))
        .filter(|(t, _)| *t >= start)
        .collect();
    points.sort_by_key(|(t, _)| *t);
    if points.is_empty() {
        painter.text(rect.center(), Align2::CENTER_CENTER, "No readings yet", FontId::proportional(14.0), visuals.weak_text_color());
        return;
    }

    let low = s.effective_target_low(snap.target_low_mg_dl);
    let high = s.effective_target_high(snap.target_high_mg_dl);
    let max_v = points.iter().map(|(_, v)| *v).fold(high, f64::max).max(250.0) + 20.0;
    let min_v = 40.0f64;
    let plot = rect.shrink2(Vec2::new(36.0, 14.0));
    let to_pos = |t: i64, v: f64| -> Pos2 {
        let x = plot.left() + plot.width() * ((t - start) as f32 / (12.0 * 3600.0));
        let y = plot.bottom() - plot.height() * (((v - min_v) / (max_v - min_v)) as f32);
        Pos2::new(x, y)
    };

    // Target band
    let band = Rect::from_x_y_ranges(plot.x_range(), to_pos(start, high).y..=to_pos(start, low).y);
    painter.rect_filled(band, CornerRadius::ZERO, Color32::from_rgba_unmultiplied(34, 197, 94, 40));

    // Y axis labels
    for v in [low, high] {
        let y = to_pos(start, v).y;
        painter.text(
            Pos2::new(rect.left() + 4.0, y),
            Align2::LEFT_CENTER,
            format_glucose_value(v, &s.result_unit),
            FontId::proportional(11.0),
            visuals.weak_text_color(),
        );
    }
    // X axis labels every 3h
    for h in (0..=12).step_by(3) {
        let t = start + h * 3600;
        let x = to_pos(t, min_v).x;
        let label = chrono::DateTime::from_timestamp(t, 0)
            .map(|d| d.with_timezone(&chrono::Local).format("%-I %p").to_string())
            .unwrap_or_default();
        painter.text(Pos2::new(x, rect.bottom() - 2.0), Align2::CENTER_BOTTOM, label, FontId::proportional(10.0), visuals.weak_text_color());
    }

    let line: Vec<Pos2> = points.iter().map(|(t, v)| to_pos(*t, *v)).collect();
    painter.add(Shape::line(line.clone(), Stroke::new(2.0, visuals.text_color())));
    for ((_, v), pos) in points.iter().zip(&line) {
        let st = GlucoseRangeStatus::classify(*v, low, high);
        painter.circle_filled(*pos, 2.5, hex_color(st.color_hex()));
    }

    // Hover readout: nearest point
    if let Some(hover) = response.hover_pos() {
        if let Some((i, pos)) = line
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1.x - hover.x).abs().total_cmp(&(b.1.x - hover.x).abs()))
        {
            let (t, v) = points[i];
            painter.circle_stroke(*pos, 5.0, Stroke::new(2.0, visuals.text_color()));
            let time = chrono::DateTime::from_timestamp(t, 0)
                .map(|d| d.with_timezone(&chrono::Local).format("%-I:%M %p").to_string())
                .unwrap_or_default();
            let text = format!("{} {} at {time}", format_glucose_value(v, &s.result_unit), s.result_unit);
            let galley = painter.layout_no_wrap(text, FontId::proportional(12.0), visuals.text_color());
            let mut box_rect = Rect::from_min_size(*pos + Vec2::new(10.0, -28.0), galley.size() + Vec2::splat(8.0));
            if box_rect.right() > rect.right() {
                box_rect = box_rect.translate(Vec2::new(-box_rect.width() - 20.0, 0.0));
            }
            painter.rect_filled(box_rect, CornerRadius::same(4), visuals.window_fill);
            painter.galley(box_rect.min + Vec2::splat(4.0), galley, visuals.text_color());
        }
    }
    let _ = convert_mg_dl_to_unit; // unit conversion handled by format_glucose_value
}

impl LibreLinkUpApp {
    fn about_view(&mut self, ui: &mut egui::Ui, state: &Shared) {
        ui.heading("LibreLinkUp Rust Desktop for Windows");
        ui.label(format!("Version {} • Native Rust + egui/eframe (Windows 10/11 x64)", env!("CARGO_PKG_VERSION")));
        ui.add_space(8.0);
        ui.label(
            "Continuous blood glucose monitor for LibreLinkUp featuring Windows DPAPI + AES-256-GCM \
             encrypted credentials, system tray glucose badge & tooltip, interactive 12-hour CGM chart, \
             stale-reading & urgent-low/snooze alert engine, and an always-on-top desktop overlay \
             with scroll-wheel transparency and quick action buttons.",
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

/// Floating always-on-top overlay. Drag to move; scroll wheel adjusts transparency; right-click or buttons for options.
fn overlay_ui(ui: &mut egui::Ui, shared: &SharedState, tx: &UnboundedSender<Command>, ui_handle: &Arc<UiHandle>) {
    let state = shared.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let s = &state.settings;
    crate::platform::set_overlay_all_desktops(s.overlay_on_all_desktops);
    let snap = &state.snapshot;
    let (label, color_hex, stale, age_text) = reading_status(&state);

    let ctx = ui.ctx().clone();
    let rect = ui.max_rect();

    // Adjust transparency with mouse scroll wheel when hovering the overlay.
    let is_hovered = ctx.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)));
    let scroll_y = if is_hovered {
        ctx.input(|i| {
            let mut wheel_y = 0.0;
            for event in &i.raw.events {
                if let egui::Event::MouseWheel { delta, .. } = event {
                    wheel_y += delta.y;
                }
            }
            if wheel_y.abs() > 0.01 {
                wheel_y
            } else if i.smooth_scroll_delta.y.abs() > 0.01 {
                i.smooth_scroll_delta.y
            } else {
                0.0
            }
        })
    } else {
        0.0
    };

    let mut current_opacity = s.overlay_opacity_percent.clamp(20, 100);
    if scroll_y != 0.0 {
        let delta = if scroll_y > 0.0 { 5 } else { -5 };
        let new_opacity = (current_opacity + delta).clamp(20, 100);
        if new_opacity != current_opacity {
            current_opacity = new_opacity;
            let mut ns = s.clone();
            ns.overlay_opacity_percent = new_opacity;
            if let Ok(mut guard) = shared.lock() {
                guard.settings = ns.clone();
            }
            let _ = tx.send(Command::SaveSettings(Box::new(ns)));
            ui_handle.repaint_all();
        }
    }

    let alpha = (current_opacity as f32 / 100.0 * 255.0) as u8;
    let status_color = hex_color(color_hex);
    let bg = if s.overlay_dark_theme {
        Color32::from_rgba_unmultiplied(15, 23, 42, alpha)
    } else {
        Color32::from_rgba_unmultiplied(status_color.r(), status_color.g(), status_color.b(), alpha)
    };

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(12), bg);
    painter.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, Color32::from_white_alpha(35)), egui::StrokeKind::Inside);
    if s.overlay_dark_theme {
        painter.rect_filled(Rect::from_min_size(rect.min, Vec2::new(6.0, rect.height())), CornerRadius::same(3), status_color);
    }

    // Drag and right-click interaction for the background
    let response = ui.interact(rect, ui.scope_id().with("overlay-drag"), Sense::click_and_drag());

    let value = format_glucose_value(snap.value_mg_dl, &s.result_unit);
    painter.text(
        rect.left_center() + Vec2::new(18.0, -8.0),
        Align2::LEFT_CENTER,
        value.clone(),
        FontId::proportional(34.0),
        Color32::WHITE,
    );
    if snap.value_mg_dl > 0.0 {
        let value_width = painter
            .layout_no_wrap(value.clone(), FontId::proportional(34.0), Color32::WHITE)
            .size()
            .x;
        paint_trend_arrow(
            painter,
            rect.left_center() + Vec2::new(18.0 + value_width + 22.0, -8.0),
            28.0,
            snap.trend_arrow,
            Color32::WHITE,
        );
    }
    let sub = if stale {
        format!("STALE • {age_text}")
    } else if age_text.is_empty() {
        format!("{} • {label}", s.result_unit)
    } else {
        format!("{} • {label} • {age_text}", s.result_unit)
    };
    painter.text(rect.left_center() + Vec2::new(18.0, 22.0), Align2::LEFT_CENTER, sub, FontId::proportional(12.0), Color32::from_white_alpha(230));

    // 2-row action button cluster on the right side (3 top buttons: 20x22, 2 bottom buttons: 32x22)
    let top_btn_size = Vec2::new(20.0, 22.0);
    let bot_btn_size = Vec2::new(32.0, 22.0);
    let btn_spacing = 4.0;
    let grid_w = top_btn_size.x * 3.0 + btn_spacing * 2.0; // 68.0
    let grid_h = top_btn_size.y * 2.0 + btn_spacing;       // 48.0
    let buttons_rect = Rect::from_min_size(
        Pos2::new(rect.max.x - 10.0 - grid_w, rect.min.y + (rect.height() - grid_h) * 0.5),
        Vec2::new(grid_w, grid_h),
    );

    let btn_theme_rect = Rect::from_min_size(buttons_rect.min, top_btn_size);
    let btn_all_desk_rect = Rect::from_min_size(
        buttons_rect.min + Vec2::new(top_btn_size.x + btn_spacing, 0.0),
        top_btn_size,
    );
    let btn_restore_rect = Rect::from_min_size(
        buttons_rect.min + Vec2::new((top_btn_size.x + btn_spacing) * 2.0, 0.0),
        top_btn_size,
    );
    let btn_menu_rect = Rect::from_min_size(
        buttons_rect.min + Vec2::new(0.0, top_btn_size.y + btn_spacing),
        bot_btn_size,
    );
    let btn_close_rect = Rect::from_min_size(
        buttons_rect.min + Vec2::new(bot_btn_size.x + btn_spacing, top_btn_size.y + btn_spacing),
        bot_btn_size,
    );

    let theme_icon = if s.overlay_dark_theme { "🌙" } else { "🎨" };
    let theme_tooltip = if s.overlay_dark_theme {
        format!("Overlay Dark Theme ON (Click for Color Fill • Scroll wheel adjusts {}% opacity)", current_opacity)
    } else {
        format!("Overlay Color Fill ON (Click for Dark Theme • Scroll wheel adjusts {}% opacity)", current_opacity)
    };
    let btn_theme_resp = draw_overlay_button(ui, btn_theme_rect, "btn_theme", theme_icon, 10.0, &theme_tooltip, false);
    if btn_theme_resp.clicked() {
        let mut ns = s.clone();
        ns.overlay_dark_theme = !ns.overlay_dark_theme;
        if let Ok(mut guard) = shared.lock() {
            guard.settings = ns.clone();
        }
        let _ = tx.send(Command::SaveSettings(Box::new(ns)));
        ui_handle.repaint_all();
    }

    let all_desk_tooltip = if s.overlay_on_all_desktops {
        "On All Desktops: ON (Visible across all virtual desktops • Click for current desktop only)"
    } else {
        "On All Desktops: OFF (Current virtual desktop only • Click to show on all desktops)"
    };
    let btn_all_desk_resp = draw_overlay_all_desktops_button(
        ui,
        btn_all_desk_rect,
        s.overlay_on_all_desktops,
        all_desk_tooltip,
    );
    if btn_all_desk_resp.clicked() {
        let mut ns = s.clone();
        ns.overlay_on_all_desktops = !ns.overlay_on_all_desktops;
        crate::platform::set_overlay_all_desktops(ns.overlay_on_all_desktops);
        if let Ok(mut guard) = shared.lock() {
            guard.settings = ns.clone();
        }
        let _ = tx.send(Command::SaveSettings(Box::new(ns)));
        ui_handle.repaint_all();
    }

    let btn_restore_resp = draw_overlay_button(ui, btn_restore_rect, "btn_restore", "↗", 12.0, "Open LibreLinkUp Rust Desktop", false);
    if btn_restore_resp.clicked() {
        ui_handle.show_main_window();
    }

    let btn_menu_resp = draw_overlay_button(ui, btn_menu_rect, "btn_menu", "☰", 11.0, "Open Menu (or right-click anywhere on overlay)", false);
    egui::Popup::menu(&btn_menu_resp).show(|ui| {
        show_overlay_menu(ui, s, shared, tx, ui_handle);
    });

    let btn_close_resp = draw_overlay_button(ui, btn_close_rect, "btn_close", "✕", 11.0, "Hide Overlay", true);
    if btn_close_resp.clicked() {
        let mut ns = s.clone();
        ns.overlay_visible = false;
        if let Ok(mut guard) = shared.lock() {
            guard.settings = ns.clone();
        }
        let _ = tx.send(Command::SaveSettings(Box::new(ns)));
        ui_handle.repaint_all();
    }

    let pointer_pos = ctx.input(|i| i.pointer.hover_pos().or_else(|| i.pointer.interact_pos()));
    let on_buttons = pointer_pos.is_some_and(|p| buttons_rect.contains(p));

    if response.drag_started() && !on_buttons {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if (response.drag_stopped() || response.clicked()) && !on_buttons {
        // Persist the new position.
        if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
            let mut ns = s.clone();
            ns.overlay_x = outer.min.x.round() as i32;
            ns.overlay_y = outer.min.y.round() as i32;
            if ns != *s {
                if let Ok(mut guard) = shared.lock() {
                    guard.settings = ns.clone();
                }
                let _ = tx.send(Command::SaveSettings(Box::new(ns)));
            }
        }
    }
    if response.double_clicked() && !on_buttons {
        ui_handle.show_main_window();
    }
    if !on_buttons {
        response.context_menu(|ui| {
            show_overlay_menu(ui, s, shared, tx, ui_handle);
        });
    }

    // Re-render periodically so the reading age stays current.
    ctx.request_repaint_after(std::time::Duration::from_secs(30));
}

fn draw_overlay_button(
    ui: &mut egui::Ui,
    rect: Rect,
    id_str: &str,
    icon: &str,
    font_size: f32,
    tooltip: &str,
    is_close: bool,
) -> egui::Response {
    let id = ui.make_persistent_id(id_str);
    let resp = ui.interact(rect, id, Sense::click());
    let painter = ui.painter();
    let hovered = resp.hovered();
    let bg = if hovered {
        if is_close {
            Color32::from_rgba_unmultiplied(239, 68, 68, 170)
        } else {
            Color32::from_white_alpha(70)
        }
    } else {
        Color32::from_white_alpha(35)
    };
    let border = if hovered {
        Color32::from_white_alpha(120)
    } else {
        Color32::from_white_alpha(65)
    };
    painter.rect_filled(rect, CornerRadius::same(6), bg);
    painter.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, border), egui::StrokeKind::Inside);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(font_size),
        Color32::WHITE,
    );
    resp.on_hover_text(tooltip)
}

fn draw_overlay_all_desktops_button(
    ui: &mut egui::Ui,
    rect: Rect,
    on_all_desktops: bool,
    tooltip: &str,
) -> egui::Response {
    let id = ui.make_persistent_id("btn_all_desktops");
    let resp = ui.interact(rect, id, Sense::click());
    let painter = ui.painter();
    let hovered = resp.hovered();
    let bg = if on_all_desktops {
        if hovered {
            Color32::from_rgba_unmultiplied(56, 189, 248, 95)
        } else {
            Color32::from_rgba_unmultiplied(56, 189, 248, 60)
        }
    } else if hovered {
        Color32::from_white_alpha(70)
    } else {
        Color32::from_white_alpha(35)
    };
    let border = if on_all_desktops {
        Color32::from_rgb(56, 189, 248)
    } else if hovered {
        Color32::from_white_alpha(120)
    } else {
        Color32::from_white_alpha(65)
    };
    painter.rect_filled(rect, CornerRadius::same(6), bg);
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );

    let center = rect.center();
    let cell = 4.2;
    let gap = 1.8;
    let total = cell * 2.0 + gap;
    let origin = center - Vec2::splat(total * 0.5);
    let active_stroke = if on_all_desktops {
        Color32::from_rgb(56, 189, 248)
    } else {
        Color32::WHITE
    };
    let active_fill = if on_all_desktops {
        Color32::from_rgba_unmultiplied(56, 189, 248, 120)
    } else {
        Color32::WHITE
    };
    let dim_stroke = Color32::from_white_alpha(105);
    for (idx, (col, row)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
        let cell_rect = Rect::from_min_size(
            origin + Vec2::new(col as f32 * (cell + gap), row as f32 * (cell + gap)),
            Vec2::splat(cell),
        );
        let is_active = idx == 0 || on_all_desktops;
        if is_active {
            painter.rect_filled(cell_rect, CornerRadius::same(1), active_fill);
            painter.rect_stroke(
                cell_rect,
                CornerRadius::same(1),
                Stroke::new(1.0, active_stroke),
                egui::StrokeKind::Inside,
            );
        } else {
            painter.rect_stroke(
                cell_rect,
                CornerRadius::same(1),
                Stroke::new(1.0, dim_stroke),
                egui::StrokeKind::Inside,
            );
        }
    }
    resp.on_hover_text(tooltip)
}

fn show_overlay_menu(
    ui: &mut egui::Ui,
    s: &AppSettings,
    shared: &SharedState,
    tx: &UnboundedSender<Command>,
    ui_handle: &Arc<UiHandle>,
) {
    if ui.button("Show main window").clicked() {
        ui_handle.show_main_window();
        ui.close();
    }
    if ui.button("Refresh now").clicked() {
        let _ = tx.send(Command::RefreshNow);
        ui.close();
    }
    let all_desk_label = if s.overlay_on_all_desktops {
        "✓ Show on all virtual desktops"
    } else {
        "Show on all virtual desktops"
    };
    if ui.button(all_desk_label).clicked() {
        let mut ns = s.clone();
        ns.overlay_on_all_desktops = !ns.overlay_on_all_desktops;
        crate::platform::set_overlay_all_desktops(ns.overlay_on_all_desktops);
        if let Ok(mut guard) = shared.lock() {
            guard.settings = ns.clone();
        }
        let _ = tx.send(Command::SaveSettings(Box::new(ns)));
        ui_handle.repaint_all();
        ui.close();
    }
    let theme_label = if s.overlay_dark_theme {
        "Switch to color theme"
    } else {
        "Switch to dark theme"
    };
    if ui.button(theme_label).clicked() {
        let mut ns = s.clone();
        ns.overlay_dark_theme = !ns.overlay_dark_theme;
        if let Ok(mut guard) = shared.lock() {
            guard.settings = ns.clone();
        }
        let _ = tx.send(Command::SaveSettings(Box::new(ns)));
        ui_handle.repaint_all();
        ui.close();
    }
    if ui.button("Hide overlay").clicked() {
        let mut ns = s.clone();
        ns.overlay_visible = false;
        if let Ok(mut guard) = shared.lock() {
            guard.settings = ns.clone();
        }
        let _ = tx.send(Command::SaveSettings(Box::new(ns)));
        ui_handle.repaint_all();
        ui.close();
    }
}
