use super::{
    AppSettings, GlucoseRangeStatus, GlucoseSnapshot, format_glucose_value, format_target_range,
    trend_arrow_description,
};

pub const MIN_UI_ZOOM_PERCENT: i32 = 75;
pub const MAX_UI_ZOOM_PERCENT: i32 = 175;

pub fn clamp_ui_zoom_percent(zoom: i32) -> i32 {
    if zoom <= 0 {
        100
    } else {
        zoom.clamp(MIN_UI_ZOOM_PERCENT, MAX_UI_ZOOM_PERCENT)
    }
}

/// Parses a `#RRGGBB` or `RRGGBB` hex color string into `(r, g, b)`.
pub fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() != 6 {
        return None;
    }
    let val = u32::from_str_radix(clean, 16).ok()?;
    Some((
        (val >> 16) as u8,
        ((val >> 8) & 0xff) as u8,
        (val & 0xff) as u8,
    ))
}

/// Computes the WCAG 2.1 relative luminance `L` in `[0.0, 1.0]` for an sRGB color.
pub fn wcag_relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    let channel = |c: u8| -> f64 {
        let s = f64::from(c) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// Computes the WCAG 2.1 contrast ratio `(L1 + 0.05) / (L2 + 0.05)` in `[1.0, 21.0]` between two
/// `#RRGGBB` hex colors.
pub fn wcag_contrast_ratio_hex(fg_hex: &str, bg_hex: &str) -> Option<f64> {
    let (r1, g1, b1) = parse_hex_rgb(fg_hex)?;
    let (r2, g2, b2) = parse_hex_rgb(bg_hex)?;
    let l1 = wcag_relative_luminance(r1, g1, b1);
    let l2 = wcag_relative_luminance(r2, g2, b2);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    Some((lighter + 0.05) / (darker + 0.05))
}

/// Builds a complete screen-reader announcement string for the current glucose snapshot.
pub fn accessible_glucose_announcement(
    snapshot: &GlucoseSnapshot,
    settings: &AppSettings,
    reading_stale: bool,
    age_display: &str,
) -> String {
    if snapshot.value_mg_dl <= 0.0 {
        return "No blood glucose reading available yet.".to_string();
    }
    let val = format_glucose_value(snapshot.value_mg_dl, &settings.result_unit);
    let trend = trend_arrow_description(snapshot.trend_arrow);
    let low = settings.effective_target_low(snapshot.target_low_mg_dl);
    let high = settings.effective_target_high(snapshot.target_high_mg_dl);
    let status = if reading_stale {
        "Stale reading"
    } else {
        GlucoseRangeStatus::classify(snapshot.value_mg_dl, low, high).label()
    };
    let target = format_target_range(low, high, &settings.result_unit);
    let age_part = if !age_display.trim().is_empty() {
        format!(", updated {}", age_display.trim())
    } else if !snapshot.timestamp.trim().is_empty() {
        format!(", measured at {}", snapshot.timestamp.trim())
    } else {
        String::new()
    };
    format!(
        "Blood glucose {} {}, trend {}, status {}{}. Target range {}.",
        val, settings.result_unit, trend, status, age_part, target
    )
}
