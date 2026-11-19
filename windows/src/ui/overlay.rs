//! Always-on-top floating CGM desktop overlay viewport.

use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Vec2, ViewportBuilder,
    ViewportCommand, ViewportId,
};
use librelinkup_core::models::{format_glucose_value, AppSettings};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    darken_color, hex_color, paint_trend_arrow, reading_status, LibreLinkUpApp, UiHandle,
    OVERLAY_ID,
};
use crate::service::{Command, SharedState};

impl LibreLinkUpApp {
    pub(super) fn show_overlay(&self, ctx: &egui::Context) {
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
        if settings.overlay_x > -32_000 && settings.overlay_y > -32_000 {
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

fn overlay_desktop_cache() -> &'static std::sync::Mutex<Option<(u64, bool)>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Option<(u64, bool)>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

pub(super) fn reset_overlay_desktop_cache() {
    if let Ok(mut guard) = overlay_desktop_cache().lock() {
        *guard = None;
    }
}

/// Floating always-on-top overlay. Drag to move; scroll wheel adjusts transparency; right-click or buttons for options.
fn overlay_ui(ui: &mut egui::Ui, shared: &SharedState, tx: &UnboundedSender<Command>, ui_handle: &Arc<UiHandle>) {
    let state = shared.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let s = &state.settings;
    {
        let mut cache = overlay_desktop_cache().lock().unwrap_or_else(|e| e.into_inner());
        if *cache != Some((state.data_rev, s.overlay_on_all_desktops)) {
            crate::platform::set_overlay_all_desktops(s.overlay_on_all_desktops);
            *cache = Some((state.data_rev, s.overlay_on_all_desktops));
        }
    }
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

    let opacity_f = (current_opacity as f32 / 100.0).clamp(0.20, 1.0);
    ui.set_opacity(opacity_f);
    let status_color = hex_color(color_hex);
    let light_fill = darken_color(status_color, 1.42);
    let light_border = darken_color(status_color, 1.95);
    let bg = if s.overlay_dark_theme {
        Color32::from_rgb(15, 23, 42)
    } else {
        Color32::from_rgb(light_fill.r(), light_fill.g(), light_fill.b())
    };

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(12), bg);
    let stroke_col = if s.overlay_dark_theme {
        Color32::from_white_alpha(65)
    } else {
        light_border
    };
    painter.rect_stroke(rect, CornerRadius::same(12), Stroke::new(2.0, stroke_col), egui::StrokeKind::Inside);
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
        ns.overlay_dark_theme.toggle();
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
        ns.overlay_on_all_desktops.toggle();
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
            Color32::from_rgba_unmultiplied(239, 68, 68, 190)
        } else {
            Color32::from_black_alpha(130)
        }
    } else {
        Color32::from_black_alpha(86)
    };
    let border = if hovered {
        Color32::from_white_alpha(200)
    } else {
        Color32::from_white_alpha(150)
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
            Color32::from_rgba_unmultiplied(56, 189, 248, 120)
        } else {
            Color32::from_rgba_unmultiplied(56, 189, 248, 85)
        }
    } else if hovered {
        Color32::from_black_alpha(130)
    } else {
        Color32::from_black_alpha(86)
    };
    let border = if on_all_desktops {
        Color32::from_rgb(56, 189, 248)
    } else if hovered {
        Color32::from_white_alpha(200)
    } else {
        Color32::from_white_alpha(150)
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
        ns.overlay_on_all_desktops.toggle();
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
        ns.overlay_dark_theme.toggle();
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
