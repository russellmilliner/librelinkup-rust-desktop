//! Main window (egui) and floating overlay viewport.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

mod alerts;
mod chart;
mod dashboard;
mod dialogs;
mod login;
mod medications;
mod overlay;
mod settings;

pub use chart::paint_trend_arrow;

use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, ViewportCommand, ViewportId};
use librelinkup_core::analytics::{GlycemicStats, StatsWindow};
use librelinkup_core::api::normalize_country_code;
use librelinkup_core::event_log::{get_all_events, get_event_seq};
use librelinkup_core::medications::MedicationSchedule;
use librelinkup_core::models::{
    AppSettings, GlucoseRangeStatus, STALE_COLOR_HEX, accessible_glucose_announcement,
    clamp_ui_zoom_percent, format_glucose_value, format_reading_age, parse_libreview_timestamp,
    trend_arrow_symbol,
};
use librelinkup_core::session::{effective_reading_age_minutes, format_snooze_until};
use tokio::sync::mpsc::UnboundedSender;
use zeroize::Zeroize;

use crate::service::{Command, Shared, SharedState, now_epoch};
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
        self.ctx
            .send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(true));
        self.ctx
            .send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus);
        crate::platform::show_window(self.hwnd.load(Ordering::SeqCst), true);
        self.ctx.request_repaint();
    }

    pub fn hide_main_window(&self) {
        self.window_visible.store(false, Ordering::SeqCst);
        self.ctx
            .send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(false));
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
        self.ctx
            .send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(true));
        self.ctx
            .send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        self.ctx.request_repaint();
    }

    pub fn repaint_all(&self) {
        self.ctx.request_repaint();
        self.ctx
            .request_repaint_of(ViewportId::from_hash_of(OVERLAY_ID));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    Dashboard,
    Medications,
    Settings,
    Alerts,
    Account,
    Logs,
    About,
}

type StatsCacheKey = (u64, StatsWindow, u64, u64, i64);

pub struct LibreLinkUpApp {
    pub(super) shared: SharedState,
    pub(super) tx: UnboundedSender<Command>,
    pub(super) ui_handle: Arc<UiHandle>,
    pub(super) tab: Tab,
    pub(super) login_email: String,
    pub(super) login_password: String,
    pub(super) login_region: String,
    pub(super) ha_url_input: String,
    pub(super) ha_token_input: String,
    pub(super) ha_entity_input: String,
    pub(super) med_editing_id: String,
    pub(super) med_name: String,
    pub(super) med_dosage: String,
    pub(super) med_schedule: MedicationSchedule,
    pub(super) med_weekday: u8,
    pub(super) med_times: String,
    pub(super) med_reminder: bool,
    pub(super) log_filter: String,
    pub(super) log_min_level: usize,
    pub(super) last_theme_key: Option<(String, i32, bool, bool)>,
    pub(super) cached_stats: Option<(StatsCacheKey, GlycemicStats)>,
    pub(super) cached_graph_key: Option<(u64, i64)>,
    pub(super) cached_graph_points: Vec<(i64, f64)>,
    pub(super) cached_logs_key: Option<(u64, usize, String)>,
    pub(super) cached_log_rows: Vec<(u8, String)>,
    pub(super) pending_sound_rx: Option<std::sync::mpsc::Receiver<Option<std::path::PathBuf>>>,
    pub(super) pending_export_rx: Option<std::sync::mpsc::Receiver<()>>,
}

pub(super) const LOGIN_PW_EDIT_SALT: &str = "llu_login_password_edit";
pub(super) const LOGIN_HA_TOK_EDIT_SALT: &str = "llu_login_ha_token_edit";
pub(super) const SETTINGS_HA_TOK_EDIT_SALT: &str = "llu_settings_ha_token_edit";

impl Drop for LibreLinkUpApp {
    fn drop(&mut self) {
        self.login_password.zeroize();
        self.ha_token_input.zeroize();
    }
}

pub(super) fn clear_text_edit_undoer(ctx: &egui::Context, id: egui::Id) {
    if let Some(mut st) = egui::TextEdit::load_state(ctx, id) {
        st.clear_undoer();
        st.store(ctx, id);
    }
}
impl LibreLinkUpApp {
    pub fn new(
        shared: SharedState,
        tx: UnboundedSender<Command>,
        ui_handle: Arc<UiHandle>,
    ) -> Self {
        let (region, ha_url, ha_entity) = shared
            .lock()
            .map(|s| {
                (
                    normalize_country_code(&s.settings.country),
                    s.settings.ha_url.clone(),
                    s.settings.ha_sensor_entity_id.clone(),
                )
            })
            .unwrap_or_else(|_| ("us".into(), String::new(), String::new()));
        Self {
            shared,
            tx,
            ui_handle,
            tab: Tab::Dashboard,
            login_email: String::new(),
            login_password: String::new(),
            login_region: if region.is_empty() {
                "us".into()
            } else {
                region
            },
            ha_url_input: ha_url,
            ha_token_input: String::new(),
            ha_entity_input: ha_entity,
            med_editing_id: String::new(),
            med_name: String::new(),
            med_dosage: String::new(),
            med_schedule: MedicationSchedule::Daily,
            med_weekday: 0,
            med_times: "08:00".to_string(),
            med_reminder: true,
            log_filter: String::new(),
            log_min_level: 0,
            last_theme_key: None,
            cached_stats: None,
            cached_graph_key: None,
            cached_graph_points: Vec::new(),
            cached_logs_key: None,
            cached_log_rows: Vec::new(),
            pending_sound_rx: None,
            pending_export_rx: None,
        }
    }

    pub(super) fn compute_cached_stats(&mut self, state: &Shared, now: i64) -> GlycemicStats {
        let low = state
            .settings
            .effective_target_low(state.snapshot.target_low_mg_dl);
        let high = state
            .settings
            .effective_target_high(state.snapshot.target_high_mg_dl);
        let key = (
            state.data_rev,
            state.stats_window,
            low.to_bits(),
            high.to_bits(),
            now / 60,
        );
        if let Some((cached_key, ref stats)) = self.cached_stats
            && cached_key == key
        {
            return stats.clone();
        }
        let stats = state.glucose_history.compute_stats(
            state.stats_window,
            now,
            low,
            high,
            Some(&state.snapshot),
        );
        self.cached_stats = Some((key, stats.clone()));
        stats
    }

    pub(super) fn cached_graph_points_for(&mut self, state: &Shared, now: i64) -> &[(i64, f64)] {
        let key = (state.data_rev, now / 60);
        if self.cached_graph_key != Some(key) {
            let start = now - 12 * 3600;
            let mut points: Vec<(i64, f64)> = state
                .snapshot
                .graph_points
                .iter()
                .filter_map(|p| {
                    parse_libreview_timestamp(&p.timestamp, false).map(|t| (t, p.value_mg_dl))
                })
                .filter(|(t, _)| *t >= start)
                .collect();
            points.sort_by_key(|(t, _)| *t);
            self.cached_graph_points = points;
            self.cached_graph_key = Some(key);
        }
        &self.cached_graph_points
    }

    pub(super) fn ensure_cached_logs(&mut self) {
        let seq = get_event_seq();
        let filter_lower = self.log_filter.to_ascii_lowercase();
        let key = (seq, self.log_min_level, filter_lower);
        if self.cached_logs_key.as_ref() == Some(&key) {
            return;
        }
        let rank = |l: &str| match l {
            "ERROR" => 3,
            "WARN" => 2,
            "INFO" => 1,
            _ => 0,
        };
        let min = key.1;
        let query = &key.2;
        self.cached_log_rows.clear();
        for e in get_all_events().iter().rev() {
            if rank(&e.level) < min {
                continue;
            }
            let line = format!(
                "{}  [{}] {}: {}",
                e.timestamp, e.level, e.category, e.message
            );
            if !query.is_empty() && !line.to_ascii_lowercase().contains(query) {
                continue;
            }
            let level_tag = match e.level.as_str() {
                "ERROR" => 2,
                "WARN" => 1,
                _ => 0,
            };
            self.cached_log_rows.push((level_tag, line));
        }
        self.cached_logs_key = Some(key);
    }

    pub(super) fn snapshot(&self) -> Shared {
        self.shared
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub(super) fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    pub(super) fn save_settings(&self, settings: AppSettings) {
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
    let h = hex.trim_prefix('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0x22c55e);
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// (label, color hex, stale?, age text) for the current reading.
pub fn reading_status(state: &Shared) -> (&'static str, &'static str, bool, String) {
    let s = &state.settings;
    let snap = &state.snapshot;
    let age = effective_reading_age_minutes(snap, now_epoch(), state.last_successful_fetch_epoch);
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
    let badge = if s.show_glucose_in_tray_icon && snap.value_mg_dl > 0.0 {
        value.clone()
    } else {
        String::new()
    };
    let mut tooltip = if !s.show_glucose_in_tray_tooltip {
        "LibreLinkUp Rust Desktop".to_string()
    } else if snap.value_mg_dl <= 0.0 {
        "LibreLinkUp — waiting for glucose reading".to_string()
    } else {
        format!(
            "{value} {} {} ({label})\n{}",
            s.result_unit,
            trend_arrow_symbol(snap.trend_arrow),
            if age_text.is_empty() {
                snap.timestamp.clone()
            } else {
                age_text.clone()
            }
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
        if self.ui_handle.window_visible.load(Ordering::SeqCst)
            && let Some(outer) = ctx.input(|i| i.viewport().outer_rect)
        {
            let wx = outer.min.x.round() as i32;
            let wy = outer.min.y.round() as i32;
            let ww = outer.width().round() as i32;
            let wh = outer.height().round() as i32;
            if wx > -32_000 && wy > -32_000 && ww >= 640 && wh >= 480 {
                let updated = self.shared.lock().ok().and_then(|mut s| {
                    if (s.settings.window_x - wx).abs() > 2
                        || (s.settings.window_y - wy).abs() > 2
                        || (s.settings.window_width - ww).abs() > 2
                        || (s.settings.window_height - wh).abs() > 2
                    {
                        s.settings.window_x = wx;
                        s.settings.window_y = wy;
                        s.settings.window_width = ww;
                        s.settings.window_height = wh;
                        Some(s.settings.clone())
                    } else {
                        None
                    }
                });
                if let Some(new_settings) = updated {
                    self.send(Command::SaveSettings(Box::new(new_settings)));
                }
            }
        }
        // Closing the main window hides it to the tray; Quit (tray menu) really exits.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.ui_handle.quitting.load(Ordering::SeqCst)
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.ui_handle.hide_main_window();
        }
        if !self
            .shared
            .lock()
            .map(|s| s.settings.overlay_visible)
            .unwrap_or(false)
        {
            overlay::reset_overlay_desktop_cache();
        }
        // Reading age / snooze countdowns change over time even without new data.
        ctx.request_repaint_after(std::time::Duration::from_secs(30));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // lets the overlay viewport be transparent
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let state = self.snapshot();
        let theme_key = (
            state.settings.theme_mode.clone(),
            state.settings.ui_text_scale,
            state.settings.high_contrast_mode,
            ui.visuals().dark_mode,
        );
        if self.last_theme_key.as_ref() != Some(&theme_key) {
            apply_theme(
                ui.ctx(),
                &state.settings.theme_mode,
                state.settings.ui_text_scale,
                state.settings.high_contrast_mode,
            );
            self.last_theme_key = Some((
                theme_key.0,
                theme_key.1,
                theme_key.2,
                ui.visuals().dark_mode,
            ));
        }
        self.handle_keyboard_shortcuts(ui.ctx(), &state);

        egui::Panel::top("header").show(ui, |ui| self.header(ui, &state));
        egui::Panel::bottom("footer").show(ui, |ui| self.footer(ui, &state));

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
            if let Some(ref upd) = state.update_info
                && upd.update_available
            {
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
                                .add_enabled(
                                    !state.installing_update,
                                    egui::Button::new(inst_label),
                                )
                                .clicked()
                            {
                                self.send(Command::InstallUpdate);
                            }
                            ui.hyperlink_to(
                                RichText::new("View Release Notes")
                                    .color(Color32::from_rgb(224, 242, 254))
                                    .underline(),
                                &upd.release_url,
                            );
                        });
                    });
                ui.add_space(6.0);
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match self.tab {
                    Tab::Dashboard if !state.logged_in => self.login_view(ui, &state),
                    Tab::Dashboard => self.dashboard(ui, &state),
                    Tab::Medications => self.medications_view(ui, &state),
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

pub fn darken_color(c: Color32, factor: f32) -> Color32 {
    let f = factor.max(1.0);
    Color32::from_rgb(
        ((c.r() as f32) / f).round().clamp(0.0, 255.0) as u8,
        ((c.g() as f32) / f).round().clamp(0.0, 255.0) as u8,
        ((c.b() as f32) / f).round().clamp(0.0, 255.0) as u8,
    )
}

fn apply_theme(ctx: &egui::Context, mode: &str, ui_text_scale: i32, high_contrast: bool) {
    let pref = match mode {
        "dark" => egui::ThemePreference::Dark,
        "light" => egui::ThemePreference::Light,
        _ => egui::ThemePreference::System,
    };
    ctx.set_theme(pref);
    let zoom = (clamp_ui_zoom_percent(ui_text_scale) as f32) / 100.0;
    if (ctx.zoom_factor() - zoom).abs() > 0.005 {
        ctx.set_zoom_factor(zoom);
    }
    ctx.all_styles_mut(|style| {
        style.spacing.scroll.bar_width = 12.0;
        style.spacing.scroll.floating = false;
        style.spacing.scroll.foreground_color = true;
        style.spacing.scroll.dormant_background_opacity = 0.55;
        style.spacing.scroll.active_background_opacity = 0.80;
        style.spacing.scroll.interact_background_opacity = 0.90;
        style.spacing.scroll.dormant_handle_opacity = 0.85;
        style.spacing.scroll.active_handle_opacity = 0.98;
        style.spacing.scroll.interact_handle_opacity = 1.0;
        if !style.visuals.dark_mode {
            style.visuals.panel_fill = Color32::from_rgb(221, 229, 240);
            style.visuals.window_fill = Color32::from_rgb(255, 255, 255);
            style.visuals.faint_bg_color = Color32::from_rgb(237, 242, 247);
            style.visuals.extreme_bg_color = Color32::from_rgb(248, 250, 252);
            style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(
                if high_contrast { 2.0 } else { 1.5 },
                if high_contrast {
                    Color32::from_rgb(15, 23, 42)
                } else {
                    Color32::from_rgb(100, 116, 139)
                },
            );
            style.visuals.widgets.noninteractive.fg_stroke =
                Stroke::new(1.0, Color32::from_rgb(15, 23, 42));
            style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(226, 232, 240);
            style.visuals.widgets.inactive.bg_stroke = Stroke::new(
                if high_contrast { 2.0 } else { 1.0 },
                if high_contrast {
                    Color32::from_rgb(15, 23, 42)
                } else {
                    Color32::from_rgb(100, 116, 139)
                },
            );
            style.visuals.widgets.inactive.fg_stroke =
                Stroke::new(1.0, Color32::from_rgb(15, 23, 42));
        } else if high_contrast {
            style.visuals.widgets.noninteractive.bg_stroke =
                Stroke::new(2.0, Color32::from_rgb(226, 232, 240));
            style.visuals.widgets.inactive.bg_stroke =
                Stroke::new(1.5, Color32::from_rgb(203, 213, 225));
        }
    });
}

impl LibreLinkUpApp {
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context, state: &Shared) {
        let mut new_zoom: Option<i32> = None;
        let mut refresh_now = false;
        let mut switch_tab: Option<Tab> = None;
        let cur_zoom = clamp_ui_zoom_percent(state.settings.ui_text_scale);
        ctx.input(|i| {
            if i.modifiers.ctrl {
                if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                    new_zoom = Some(clamp_ui_zoom_percent(cur_zoom + 10));
                } else if i.key_pressed(egui::Key::Minus) {
                    new_zoom = Some(clamp_ui_zoom_percent(cur_zoom - 10));
                } else if i.key_pressed(egui::Key::Num0) {
                    new_zoom = Some(100);
                } else if i.key_pressed(egui::Key::R) {
                    refresh_now = true;
                }
            }
            if i.key_pressed(egui::Key::F5) {
                refresh_now = true;
            }
            if i.modifiers.alt && !i.modifiers.ctrl {
                if i.key_pressed(egui::Key::Num1) {
                    switch_tab = Some(Tab::Dashboard);
                } else if i.key_pressed(egui::Key::Num2) {
                    switch_tab = Some(Tab::Medications);
                } else if i.key_pressed(egui::Key::Num3) {
                    switch_tab = Some(Tab::Settings);
                } else if i.key_pressed(egui::Key::Num4) {
                    switch_tab = Some(Tab::Alerts);
                } else if i.key_pressed(egui::Key::Num5) {
                    switch_tab = Some(Tab::Account);
                } else if i.key_pressed(egui::Key::Num6) {
                    switch_tab = Some(Tab::Logs);
                } else if i.key_pressed(egui::Key::Num7) {
                    switch_tab = Some(Tab::About);
                }
            }
        });
        if let Some(z) = new_zoom {
            let mut ns = state.settings.clone();
            ns.ui_text_scale = z;
            self.save_settings(ns);
        }
        if refresh_now && state.logged_in && !state.busy && !state.demo {
            self.send(Command::RefreshNow);
        }
        if let Some(t) = switch_tab {
            self.tab = t;
        }
    }

    fn footer(&mut self, ui: &mut egui::Ui, state: &Shared) {
        let (_, _, stale, age_text) = reading_status(state);
        let summary = if state.logged_in {
            accessible_glucose_announcement(&state.snapshot, &state.settings, stale, &age_text)
        } else {
            state.status.clone()
        };
        let source_label = if state.demo {
            "Sample Data (Demo)".to_string()
        } else if state.settings.data_source == "home_assistant" {
            let trimmed = state.settings.ha_url.trim();
            let host = trimmed
                .strip_prefix("https://")
                .or_else(|| trimmed.strip_prefix("http://"))
                .unwrap_or(trimmed)
                .split('/')
                .next()
                .unwrap_or("");
            if host.is_empty() {
                "Home Assistant".to_string()
            } else {
                format!("Home Assistant ({host})")
            }
        } else {
            format!("LibreLinkUp ({})", state.settings.country.to_uppercase())
        };
        let (conn_badge, conn_color) = if state.busy {
            (
                format!("⟳ Syncing • {source_label}"),
                Color32::from_rgb(234, 179, 8),
            )
        } else if state.demo {
            (
                format!("● Connected • {source_label}"),
                Color32::from_rgb(2, 132, 199),
            )
        } else if state.logged_in {
            if stale {
                (
                    format!("⚠ Connected • {source_label}"),
                    Color32::from_rgb(245, 158, 11),
                )
            } else {
                (
                    format!("● Connected • {source_label}"),
                    Color32::from_rgb(34, 197, 94),
                )
            }
        } else {
            (
                format!("○ Disconnected • {source_label}"),
                Color32::from_rgb(100, 116, 139),
            )
        };
        let cur_zoom = clamp_ui_zoom_percent(state.settings.ui_text_scale);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&conn_badge)
                    .small()
                    .strong()
                    .color(conn_color),
            )
            .on_hover_text(&state.status);
            ui.separator();
            ui.label(RichText::new(&summary).small());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(cur_zoom < 175, egui::Button::new("+").small())
                    .on_hover_text("Zoom in (Ctrl++)")
                    .clicked()
                {
                    let mut ns = state.settings.clone();
                    ns.ui_text_scale = clamp_ui_zoom_percent(cur_zoom + 10);
                    self.save_settings(ns);
                }
                if ui
                    .button(RichText::new(format!("{cur_zoom}%")).small().strong())
                    .on_hover_text("Reset UI zoom to 100% (Ctrl+0)")
                    .clicked()
                {
                    let mut ns = state.settings.clone();
                    ns.ui_text_scale = 100;
                    self.save_settings(ns);
                }
                if ui
                    .add_enabled(cur_zoom > 75, egui::Button::new("−").small())
                    .on_hover_text("Zoom out (Ctrl+-)")
                    .clicked()
                {
                    let mut ns = state.settings.clone();
                    ns.ui_text_scale = clamp_ui_zoom_percent(cur_zoom - 10);
                    self.save_settings(ns);
                }
                let mut hc = state.settings.high_contrast_mode;
                if ui
                    .toggle_value(&mut hc, RichText::new("◐ High Contrast").small())
                    .on_hover_text("Toggle High Contrast accessibility borders and badges")
                    .changed()
                {
                    let mut ns = state.settings.clone();
                    ns.high_contrast_mode = hc;
                    self.save_settings(ns);
                }
            });
        });
    }
    fn header(&mut self, ui: &mut egui::Ui, state: &Shared) {
        ui.horizontal_wrapped(|ui| {
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
                (Tab::Medications, "💊 Medications"),
                (Tab::Settings, "⚙ Settings"),
                (Tab::Alerts, "🔔 Alerts"),
                (Tab::Account, "👤 Account"),
                (Tab::Logs, "📜 Logs"),
                (Tab::About, "ℹ About"),
            ] {
                ui.selectable_value(&mut self.tab, tab, label);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let refresh = ui.add_enabled(
                    state.logged_in && !state.busy && !state.demo,
                    egui::Button::new("🔄"),
                );
                if refresh.on_hover_text("Refresh glucose now").clicked() {
                    self.send(Command::RefreshNow);
                }
                if state.busy {
                    ui.spinner();
                }
            });
        });
    }
}

pub(super) fn region_combo(ui: &mut egui::Ui, region: &mut String) {
    let current = REGIONS
        .iter()
        .find(|(c, _)| c == region)
        .map(|(_, n)| *n)
        .unwrap_or("United States");
    egui::ComboBox::from_id_salt("region")
        .selected_text(format!("{current} ({region})"))
        .width(260.0)
        .show_ui(ui, |ui| {
            for (code, name) in REGIONS {
                ui.selectable_value(region, code.to_string(), format!("{name} ({code})"));
            }
        });
}
