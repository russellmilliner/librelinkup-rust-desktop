//! Vector trend arrow renderer and interactive 12-hour CGM trend chart.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2,
};
use librelinkup_core::models::{GlucoseRangeStatus, convert_mg_dl_to_unit, format_glucose_value};

use super::hex_color;
use crate::service::Shared;

/// Draws the LibreLinkUp trend arrow (1 = falling quickly … 5 = rising quickly) as a vector
/// shape, so it renders identically regardless of installed fonts.
pub fn paint_trend_arrow(
    painter: &egui::Painter,
    center: Pos2,
    size: f32,
    trend: i32,
    color: Color32,
) {
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
        vec![
            tip,
            head_base + normal * head_len * 0.6,
            head_base - normal * head_len * 0.6,
        ],
        color,
        Stroke::NONE,
    ));
}

/// 12-hour trend graph with target band and hover readout.
pub(super) fn glucose_graph(ui: &mut egui::Ui, state: &Shared, points: &[(i64, f64)], now: i64) {
    let s = &state.settings;
    let snap = &state.snapshot;
    let width = ui.available_width().max(300.0);
    let (response, painter) = ui.allocate_painter(Vec2::new(width, 220.0), Sense::hover());
    let rect = response.rect;
    let visuals = ui.visuals().clone();
    painter.rect_filled(rect, CornerRadius::same(6), visuals.extreme_bg_color);

    let start = now - 12 * 3600;
    if points.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No readings yet",
            FontId::proportional(14.0),
            visuals.weak_text_color(),
        );
        return;
    }

    let low = s.effective_target_low(snap.target_low_mg_dl);
    let high = s.effective_target_high(snap.target_high_mg_dl);
    let max_v = points
        .iter()
        .map(|(_, v)| *v)
        .fold(high, f64::max)
        .max(250.0)
        + 20.0;
    let min_v = 40.0f64;
    let plot = rect.shrink2(Vec2::new(36.0, 14.0));
    let to_pos = |t: i64, v: f64| -> Pos2 {
        let x = plot.left() + plot.width() * ((t - start) as f32 / (12.0 * 3600.0));
        let y = plot.bottom() - plot.height() * (((v - min_v) / (max_v - min_v)) as f32);
        Pos2::new(x, y)
    };

    // Target band
    let band = Rect::from_x_y_ranges(plot.x_range(), to_pos(start, high).y..=to_pos(start, low).y);
    painter.rect_filled(
        band,
        CornerRadius::ZERO,
        Color32::from_rgba_unmultiplied(34, 197, 94, 40),
    );

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
        painter.text(
            Pos2::new(x, rect.bottom() - 2.0),
            Align2::CENTER_BOTTOM,
            label,
            FontId::proportional(10.0),
            visuals.weak_text_color(),
        );
    }

    let line: Vec<Pos2> = points.iter().map(|(t, v)| to_pos(*t, *v)).collect();
    painter.add(Shape::line(
        line.clone(),
        Stroke::new(2.0, visuals.text_color()),
    ));
    for ((_, v), pos) in points.iter().zip(&line) {
        let st = GlucoseRangeStatus::classify(*v, low, high);
        painter.circle_filled(*pos, 2.5, hex_color(st.color_hex()));
    }

    // Logged Medication Dose Markers (#a855f7 diamond + vertical guide line)
    let dose_color = Color32::from_rgb(168, 85, 247);
    let recent_doses = state.medication_store.doses_in_window(start, now);
    for dose in &recent_doses {
        let g_val = dose.glucose_mg_dl_at_dose.unwrap_or_else(|| {
            points
                .iter()
                .min_by_key(|(t, _)| (*t - dose.taken_at_epoch).abs())
                .map(|(_, v)| *v)
                .unwrap_or(110.0)
        });
        let d_pos = to_pos(dose.taken_at_epoch, g_val);
        painter.line_segment(
            [
                Pos2::new(d_pos.x, plot.top()),
                Pos2::new(d_pos.x, plot.bottom()),
            ],
            Stroke::new(1.5, Color32::from_rgba_unmultiplied(168, 85, 247, 130)),
        );
        let diamond = vec![
            Pos2::new(d_pos.x, d_pos.y - 6.0),
            Pos2::new(d_pos.x + 6.0, d_pos.y),
            Pos2::new(d_pos.x, d_pos.y + 6.0),
            Pos2::new(d_pos.x - 6.0, d_pos.y),
        ];
        painter.add(Shape::convex_polygon(
            diamond,
            dose_color,
            Stroke::new(1.5, Color32::WHITE),
        ));
        painter.text(
            Pos2::new(
                d_pos.x.clamp(plot.left() + 24.0, plot.right() - 24.0),
                (d_pos.y - 10.0).max(plot.top() + 8.0),
            ),
            Align2::CENTER_BOTTOM,
            format!("💊 {}", dose.medication_name),
            FontId::proportional(10.5),
            dose_color,
        );
    }

    // Hover readout: nearest point
    if let Some(hover) = response.hover_pos()
        && let Some((i, pos)) = line
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1.x - hover.x).abs().total_cmp(&(b.1.x - hover.x).abs()))
    {
        let (t, v) = points[i];
        painter.circle_stroke(*pos, 5.0, Stroke::new(2.0, visuals.text_color()));
        let time = chrono::DateTime::from_timestamp(t, 0)
            .map(|d| {
                d.with_timezone(&chrono::Local)
                    .format("%-I:%M %p")
                    .to_string()
            })
            .unwrap_or_default();
        let mut text = format!(
            "{} {} at {time}",
            format_glucose_value(v, &s.result_unit),
            s.result_unit
        );
        for dose in &recent_doses {
            if (dose.taken_at_epoch - t).abs() <= 15 * 60 {
                text.push_str(&format!(" • 💊 {} ({})", dose.medication_name, dose.dosage));
            }
        }
        let galley = painter.layout_no_wrap(text, FontId::proportional(12.0), visuals.text_color());
        let mut box_rect = Rect::from_min_size(
            *pos + Vec2::new(10.0, -28.0),
            galley.size() + Vec2::splat(8.0),
        );
        if box_rect.right() > rect.right() {
            box_rect = box_rect.translate(Vec2::new(-box_rect.width() - 20.0, 0.0));
        }
        painter.rect_filled(box_rect, CornerRadius::same(4), visuals.window_fill);
        painter.galley(
            box_rect.min + Vec2::splat(4.0),
            galley,
            visuals.text_color(),
        );
    }
    let _ = convert_mg_dl_to_unit; // unit conversion handled by format_glucose_value
}
