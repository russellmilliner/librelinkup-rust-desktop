//! System tray icon with a live glucose badge, tooltip and context menu.
//!
//! Runs on a dedicated thread with its own Win32 message pump, so it stays responsive and
//! up to date even while the main window is hidden.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const ICON_SIZE: u32 = 32;
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/icon-32.png");

/// Everything the tray shows, rebuilt by the UI layer whenever shared state changes.
#[derive(Debug, Clone, PartialEq)]
pub struct TrayView {
    /// Badge text drawn into the icon (empty = plain app icon).
    pub badge: String,
    pub badge_color: [u8; 3],
    pub tooltip: String,
    pub status_line: String,
    pub overlay_visible: bool,
    pub autostart: bool,
    pub snooze_line: String,
}

#[derive(Debug, Clone, Copy)]
pub enum TrayAction {
    ToggleWindow,
    ShowWindow,
    Refresh,
    Snooze(i32),
    SetOverlay(bool),
    SetAutostart(bool),
    CheckUpdates,
    Quit,
}

pub fn start(on_action: impl Fn(TrayAction) + Send + 'static) -> Sender<TrayView> {
    let (tx, rx) = std::sync::mpsc::channel::<TrayView>();
    std::thread::Builder::new()
        .name("llu-tray".into())
        .spawn(move || run(rx, on_action))
        .expect("spawn tray thread");
    tx
}

struct MenuIds {
    show: MenuItem,
    refresh: MenuItem,
    status: MenuItem,
    snooze_status: MenuItem,
    snooze: Vec<(MenuItem, i32)>,
    overlay: CheckMenuItem,
    autostart: CheckMenuItem,
    check_updates: MenuItem,
    quit: MenuItem,
}

fn build_menu() -> (Menu, MenuIds) {
    let menu = Menu::new();
    let status = MenuItem::new("Blood Sugar: --", false, None);
    let show = MenuItem::new("Show LibreLinkUp", true, None);
    let refresh = MenuItem::new("Refresh Reading Now", true, None);
    let snooze_menu = Submenu::new("Snooze Alerts", true);
    let snooze_status = MenuItem::new("Not snoozed", false, None);
    let snooze: Vec<(MenuItem, i32)> = [("30 minutes", 30), ("1 hour", 60), ("2 hours", 120), ("Cancel snooze", 0)]
        .into_iter()
        .map(|(label, minutes)| (MenuItem::new(label, true, None), minutes))
        .collect();
    let _ = snooze_menu.append(&snooze_status);
    let _ = snooze_menu.append(&PredefinedMenuItem::separator());
    for (item, _) in &snooze {
        let _ = snooze_menu.append(item);
    }
    let overlay = CheckMenuItem::new("Show Floating Overlay", true, false, None);
    let autostart = CheckMenuItem::new("Start at Login", true, false, None);
    let check_updates = MenuItem::new("Check for Updates", true, None);
    let quit = MenuItem::new("Quit", true, None);

    let _ = menu.append(&status);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&show);
    let _ = menu.append(&refresh);
    let _ = menu.append(&snooze_menu);
    let _ = menu.append(&overlay);
    let _ = menu.append(&autostart);
    let _ = menu.append(&check_updates);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);
    (menu, MenuIds { show, refresh, status, snooze_status, snooze, overlay, autostart, check_updates, quit })
}

fn run(rx: Receiver<TrayView>, on_action: impl Fn(TrayAction)) {
    let font = load_badge_font();
    let (menu, ids) = build_menu();
    let tray = match TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip("LibreLinkUp Rust Desktop")
        .with_icon(app_icon())
        .build()
    {
        Ok(t) => t,
        Err(e) => {
            librelinkup_core::event_log::log_error("TRAY", format!("Failed to create tray icon: {e}"));
            return;
        }
    };
    let mut tray: Option<TrayIcon> = Some(tray);
    let mut current: Option<TrayView> = None;

    loop {
        pump_messages();

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id;
            if id == *ids.quit.id() {
                tray.take(); // remove the icon before exiting so no ghost icon remains
                on_action(TrayAction::Quit);
                return;
            } else if id == *ids.show.id() {
                on_action(TrayAction::ShowWindow);
            } else if id == *ids.refresh.id() {
                on_action(TrayAction::Refresh);
            } else if id == *ids.overlay.id() {
                on_action(TrayAction::SetOverlay(ids.overlay.is_checked()));
            } else if id == *ids.autostart.id() {
                on_action(TrayAction::SetAutostart(ids.autostart.is_checked()));
            } else if id == *ids.check_updates.id() {
                on_action(TrayAction::CheckUpdates);
            } else if let Some((_, minutes)) = ids.snooze.iter().find(|(item, _)| *item.id() == id) {
                on_action(TrayAction::Snooze(*minutes));
            }
        }

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                on_action(TrayAction::ToggleWindow);
            }
        }

        // Apply only the newest view; wait briefly so the loop doesn't spin.
        let mut latest = None;
        match rx.recv_timeout(Duration::from_millis(40)) {
            Ok(view) => latest = Some(view),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        while let Ok(view) = rx.try_recv() {
            latest = Some(view);
        }
        if let (Some(view), Some(tray)) = (latest, tray.as_ref()) {
            if current.as_ref() != Some(&view) {
                apply_view(tray, &ids, &view, current.as_ref(), font.as_ref());
                current = Some(view);
            }
        }
    }
}

fn apply_view(tray: &TrayIcon, ids: &MenuIds, view: &TrayView, prev: Option<&TrayView>, font: Option<&FontArc>) {
    let icon_changed = prev.is_none_or(|p| p.badge != view.badge || p.badge_color != view.badge_color);
    if icon_changed {
        let icon = if view.badge.is_empty() {
            app_icon()
        } else {
            font.and_then(|f| render_badge(f, &view.badge, view.badge_color)).unwrap_or_else(app_icon)
        };
        let _ = tray.set_icon(Some(icon));
    }
    // Windows limits tray tooltips to 127 characters.
    let tooltip: String = view.tooltip.chars().take(127).collect();
    let _ = tray.set_tooltip(Some(tooltip));
    ids.status.set_text(&view.status_line);
    ids.snooze_status.set_text(if view.snooze_line.is_empty() { "Not snoozed" } else { &view.snooze_line });
    ids.overlay.set_checked(view.overlay_visible);
    ids.autostart.set_checked(view.autostart);
}

fn app_icon() -> Icon {
    let data = eframe::icon_data::from_png_bytes(APP_ICON_PNG).expect("valid embedded icon");
    Icon::from_rgba(data.rgba, data.width, data.height).expect("valid icon size")
}

fn load_badge_font() -> Option<FontArc> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    for name in ["segoeuib.ttf", "arialbd.ttf", "segoeui.ttf", "arial.ttf"] {
        let path = std::path::Path::new(&windir).join("Fonts").join(name);
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(font) = FontArc::try_from_vec(bytes) {
                return Some(font);
            }
        }
    }
    FontArc::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).ok()
}

/// Draws `text` in white on a rounded square of `color` (32×32 RGBA).
pub fn render_badge(font: &FontArc, text: &str, color: [u8; 3]) -> Option<Icon> {
    let size = ICON_SIZE as usize;
    let mut rgba = vec![0u8; size * size * 4];
    let radius = 6.0f32;
    for y in 0..size {
        for x in 0..size {
            let coverage = rounded_rect_coverage(x as f32 + 0.5, y as f32 + 0.5, size as f32, radius);
            if coverage > 0.0 {
                let i = (y * size + x) * 4;
                rgba[i..i + 3].copy_from_slice(&color);
                rgba[i + 3] = (coverage * 255.0) as u8;
            }
        }
    }

    // Largest font size (<= 22 px) at which the text fits within 30 px.
    let max_width = size as f32 - 2.0;
    let mut px = 22.0f32;
    let width_at = |px: f32| -> f32 {
        let scaled = font.as_scaled(PxScale::from(px));
        text.chars().map(|c| scaled.h_advance(font.glyph_id(c))).sum()
    };
    while px > 8.0 && width_at(px) > max_width {
        px -= 0.5;
    }
    let scaled = font.as_scaled(PxScale::from(px));
    let text_width = width_at(px);
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let mut x = (size as f32 - text_width) / 2.0;
    let baseline = (size as f32 + ascent + descent) / 2.0;

    for c in text.chars() {
        let glyph_id = font.glyph_id(c);
        let glyph = glyph_id.with_scale_and_position(PxScale::from(px), ab_glyph::point(x, baseline));
        x += scaled.h_advance(glyph_id);
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, cov| {
                let px_x = bounds.min.x as i32 + gx as i32;
                let px_y = bounds.min.y as i32 + gy as i32;
                if px_x < 0 || px_y < 0 || px_x >= size as i32 || px_y >= size as i32 {
                    return;
                }
                let i = (px_y as usize * size + px_x as usize) * 4;
                for ch in 0..3 {
                    let bg = rgba[i + ch] as f32;
                    rgba[i + ch] = (bg + (255.0 - bg) * cov).round() as u8;
                }
                rgba[i + 3] = rgba[i + 3].max((cov * 255.0) as u8);
            });
        }
    }
    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE).ok()
}

/// Anti-aliased coverage of a rounded square of side `size` at pixel center (px, py).
fn rounded_rect_coverage(px: f32, py: f32, size: f32, radius: f32) -> f32 {
    let cx = px.clamp(radius, size - radius);
    let cy = py.clamp(radius, size - radius);
    let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    (radius - dist + 0.5).clamp(0.0, 1.0)
}

#[cfg(windows)]
fn pump_messages() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(windows))]
fn pump_messages() {}
