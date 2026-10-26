//! Overlay geometry (PLAN.md 6.4): monitor choice, nine anchors, offsets in percent of the
//! monitor, width, scale and DPI. Pure functions on plain data, so the maths is tested
//! without a window.
//!
//! Coordinates: the overlay page works in CSS pixels (= logical pixels of its monitor). A
//! `Rect` is in CSS pixels relative to the monitor's top-left corner. Window bounds are
//! physical pixels in virtual-screen coordinates.

use serde::{Deserialize, Serialize};

use super::win32;
use crate::settings::{Anchor, OverlaySettings};

/// The design sizes of the overlay refer to a screen this many logical pixels high (PLAN.md 9.5).
pub const DESIGN_HEIGHT: f64 = 1080.0;
/// Distance of the snap points from the monitor edges, at design size.
pub const EDGE_MARGIN: f64 = 24.0;
/// Narrowest overlay in logical pixels (PLAN.md 6.4).
pub const MIN_WIDTH_CSS: f64 = 200.0;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Monitor {
    /// GDI device name, stored as `monitor_id` in the settings.
    pub id: String,
    /// 1-based position in Windows' enumeration, for labels such as "Monitor 2".
    pub number: usize,
    pub primary: bool,
    /// Physical pixels in virtual-screen coordinates.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Physical pixels per logical pixel (1.0 = 100 %).
    pub scale_factor: f64,
}

impl Monitor {
    /// Size in logical (CSS) pixels.
    pub fn css_size(&self) -> (f64, f64) {
        (self.width as f64 / self.scale_factor, self.height as f64 / self.scale_factor)
    }

    /// Keeps the overlay the same size relative to the screen: 1.0 at 1080 logical pixels
    /// on the shorter side, 1.33 at 1440.
    pub fn resolution_factor(&self) -> f64 {
        let (w, h) = self.css_size();
        w.min(h) / DESIGN_HEIGHT
    }

    pub fn edge_margin_css(&self) -> f64 {
        EDGE_MARGIN * self.resolution_factor()
    }

    pub fn label(&self) -> String {
        format!(
            "Monitor {} · {}×{}{}",
            self.number,
            self.width,
            self.height,
            if self.primary { " · Hauptbildschirm" } else { "" }
        )
    }
}

pub fn list_monitors() -> Vec<Monitor> {
    win32::monitors()
        .into_iter()
        .enumerate()
        .map(|(i, m)| Monitor {
            id: m.device,
            number: i + 1,
            primary: m.primary,
            x: m.rect.left,
            y: m.rect.top,
            width: m.rect.right - m.rect.left,
            height: m.rect.bottom - m.rect.top,
            scale_factor: m.dpi.max(1) as f64 / 96.0,
        })
        .collect()
}

/// The configured monitor; falls back to the primary one if it is missing (PLAN.md 6.4).
pub fn pick<'a>(monitors: &'a [Monitor], id: Option<&str>) -> Option<&'a Monitor> {
    id.and_then(|id| monitors.iter().find(|m| m.id == id))
        .or_else(|| monitors.iter().find(|m| m.primary))
        .or_else(|| monitors.first())
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// What the overlay page needs to lay out its content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContentLayout {
    pub width_css: f64,
    /// Factor for text and spacing: settings scale × resolution factor.
    pub ui_scale: f64,
}

pub fn content_layout(o: &OverlaySettings, m: &Monitor) -> ContentLayout {
    let (w, _) = m.css_size();
    ContentLayout {
        width_css: (o.width_pct / 100.0 * w).max(MIN_WIDTH_CSS).min(w),
        ui_scale: o.scale * m.resolution_factor(),
    }
}

/// Horizontal alignment of the content inside the overlay, following the anchor column.
pub fn align(anchor: Anchor) -> &'static str {
    match anchor.cell().0 {
        0 => "start",
        1 => "center",
        _ => "end",
    }
}

/// Where content of `size` (CSS px) goes on the monitor. Stays fully on the monitor.
pub fn place(o: &OverlaySettings, m: &Monitor, size: (f64, f64)) -> Rect {
    let (mw, mh) = m.css_size();
    let (cw, ch) = size;
    let (col, row) = o.anchor.cell();
    let x = frac(col) * (mw - cw) + o.offset_x_pct / 100.0 * mw;
    let y = frac(row) * (mh - ch) + o.offset_y_pct / 100.0 * mh;
    Rect { x: clamp_axis(x, cw, mw), y: clamp_axis(y, ch, mh), width: cw, height: ch }
}

/// Window bounds (x, y, width, height) in physical pixels for a rectangle on a monitor.
pub fn to_physical(m: &Monitor, r: &Rect) -> (i32, i32, i32, i32) {
    let s = m.scale_factor;
    let width = ((r.width * s).ceil() as i32).clamp(1, m.width);
    let height = ((r.height * s).ceil() as i32).clamp(1, m.height);
    let x = m.x + ((r.x * s).round() as i32).clamp(0, m.width - width);
    let y = m.y + ((r.y * s).round() as i32).clamp(0, m.height - height);
    (x, y, width, height)
}

/// Geometry settings that reproduce `r`, used when the edit mode saves. The anchor is the
/// third of the monitor that holds the rectangle's centre, so a bottom-right overlay stays
/// bottom-right when the resolution changes.
pub fn geometry_from_rect(m: &Monitor, r: &Rect, width_css: f64) -> (Anchor, f64, f64, f64) {
    let (mw, mh) = m.css_size();
    let third = |centre: f64, total: f64| {
        if centre < total / 3.0 {
            0
        } else if centre > total * 2.0 / 3.0 {
            2
        } else {
            1
        }
    };
    let col = third(r.x + r.width / 2.0, mw);
    let row = third(r.y + r.height / 2.0, mh);
    let offset_x = (r.x - frac(col) * (mw - r.width)) / mw * 100.0;
    let offset_y = (r.y - frac(row) * (mh - r.height)) / mh * 100.0;
    (Anchor::from_cell(col, row), round3(offset_x), round3(offset_y), round3(width_css / mw * 100.0))
}

/// Offsets that put the overlay at the snap point of `anchor`, `EDGE_MARGIN` from the edges.
pub fn snapped_offsets(anchor: Anchor, m: &Monitor) -> (f64, f64) {
    let (mw, mh) = m.css_size();
    let margin = m.edge_margin_css();
    let (col, row) = anchor.cell();
    let sign = |i: usize| 1.0 - i as f64; // left/top +, centre 0, right/bottom −
    (round3(sign(col) * margin / mw * 100.0), round3(sign(row) * margin / mh * 100.0))
}

fn frac(cell: usize) -> f64 {
    cell as f64 / 2.0
}

fn clamp_axis(pos: f64, size: f64, total: f64) -> f64 {
    if size >= total { 0.0 } else { pos.clamp(0.0, total - size) }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, width: i32, height: i32, scale_factor: f64) -> Monitor {
        Monitor { id: format!("M{x}"), number: 1, primary: x == 0, x, y, width, height, scale_factor }
    }

    fn settings(anchor: Anchor, ox: f64, oy: f64) -> OverlaySettings {
        OverlaySettings { anchor, offset_x_pct: ox, offset_y_pct: oy, ..OverlaySettings::default() }
    }

    #[test]
    fn top_centre_on_1440p() {
        let m = monitor(0, 0, 2560, 1440, 1.0);
        let o = settings(Anchor::TopCenter, 0.0, 2.0);
        let c = content_layout(&o, &m);
        assert_eq!(c.width_css, 896.0);
        assert!((c.ui_scale - 1440.0 / 1080.0).abs() < 1e-9);
        let r = place(&o, &m, (c.width_css, 60.0));
        assert_eq!((r.x, r.y), (832.0, 28.8));
        assert_eq!(to_physical(&m, &r), (832, 29, 896, 60));
    }

    #[test]
    fn bottom_right_with_dpi_and_negative_origin() {
        // Left-hand 1080p monitor at 150 %: 1280×720 logical.
        let m = monitor(-1920, 150, 1920, 1080, 1.5);
        let o = settings(Anchor::BottomRight, -2.0, -5.0);
        let r = place(&o, &m, (400.0, 50.0));
        assert!((r.x - (1280.0 - 400.0 - 25.6)).abs() < 1e-9);
        assert!((r.y - (720.0 - 50.0 - 36.0)).abs() < 1e-9);
        let (x, y, w, h) = to_physical(&m, &r);
        assert_eq!((w, h), (600, 75));
        assert_eq!(x, -1920 + 1282);
        assert_eq!(y, 150 + 951);
    }

    #[test]
    fn stays_on_the_monitor() {
        let m = monitor(0, 0, 1920, 1080, 1.0);
        let r = place(&settings(Anchor::TopLeft, -50.0, 150.0), &m, (500.0, 60.0));
        assert_eq!((r.x, r.y), (0.0, 1020.0));
        // Wider than the monitor: pinned to the left edge.
        let r = place(&settings(Anchor::MiddleCenter, 0.0, 0.0), &m, (3000.0, 60.0));
        assert_eq!(r.x, 0.0);
    }

    #[test]
    fn edit_rect_roundtrips_through_settings() {
        let m = monitor(2560, -267, 1200, 1920, 1.0);
        for (x, y) in [(10.0, 20.0), (400.0, 900.0), (700.0, 1800.0), (300.0, 1000.0)] {
            let r = Rect { x, y, width: 480.0, height: 64.0 };
            let (anchor, ox, oy, wp) = geometry_from_rect(&m, &r, 480.0);
            let o = OverlaySettings {
                anchor,
                offset_x_pct: ox,
                offset_y_pct: oy,
                width_pct: wp,
                ..OverlaySettings::default()
            };
            let back = place(&o, &m, (content_layout(&o, &m).width_css, 64.0));
            assert!((back.x - x).abs() < 0.1 && (back.y - y).abs() < 0.1, "{anchor:?}: {back:?} vs ({x},{y})");
        }
    }

    #[test]
    fn anchor_follows_the_third_of_the_centre() {
        let m = monitor(0, 0, 1920, 1080, 1.0);
        let at = |x: f64, y: f64| geometry_from_rect(&m, &Rect { x, y, width: 200.0, height: 50.0 }, 200.0).0;
        assert_eq!(at(10.0, 10.0), Anchor::TopLeft);
        assert_eq!(at(860.0, 10.0), Anchor::TopCenter);
        assert_eq!(at(1700.0, 1020.0), Anchor::BottomRight);
        assert_eq!(at(860.0, 515.0), Anchor::MiddleCenter);
    }

    #[test]
    fn snapped_offsets_keep_the_edge_margin() {
        let m = monitor(0, 0, 2560, 1440, 1.0);
        let margin = 24.0 * 1440.0 / 1080.0;
        let (ox, oy) = snapped_offsets(Anchor::BottomRight, &m);
        let r = place(&settings(Anchor::BottomRight, ox, oy), &m, (500.0, 60.0));
        assert!((2560.0 - (r.x + 500.0) - margin).abs() < 0.05);
        assert!((1440.0 - (r.y + 60.0) - margin).abs() < 0.05);
        assert_eq!(snapped_offsets(Anchor::MiddleCenter, &m), (0.0, 0.0));
    }

    #[test]
    fn missing_monitor_falls_back_to_primary() {
        let ms = [monitor(-1920, 0, 1920, 1080, 1.0), monitor(0, 0, 2560, 1440, 1.0)];
        assert_eq!(pick(&ms, Some("gone")).unwrap().x, 0);
        assert_eq!(pick(&ms, Some("M-1920")).unwrap().x, -1920);
        assert_eq!(pick(&ms, None).unwrap().x, 0);
        assert!(pick(&[], None).is_none());
    }

    #[test]
    fn width_has_a_floor() {
        let m = monitor(0, 0, 1200, 1920, 1.0);
        let o = OverlaySettings { width_pct: 10.0, ..OverlaySettings::default() };
        assert_eq!(content_layout(&o, &m).width_css, MIN_WIDTH_CSS);
    }
}
