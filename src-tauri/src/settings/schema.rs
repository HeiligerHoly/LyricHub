//! Settings schema v2 (PLAN.md 6.4, 9.7). Every struct defaults missing fields and keeps
//! unknown ones (`extra`), so a file can travel between versions and branches. Later
//! versions can add fields without a migration; renames and meaning changes need one in
//! `store::migrate` (PLAN.md 15).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub overlay: OverlaySettings,
    pub appearance: AppearanceSettings,
    pub sync: SyncSettings,
    pub behavior: BehaviorSettings,
    /// Fields this version does not know (a newer build, another branch): kept and written back.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            overlay: OverlaySettings::default(),
            appearance: AppearanceSettings::default(),
            sync: SyncSettings::default(),
            behavior: BehaviorSettings::default(),
            extra: Map::new(),
        }
    }
}

/// Overlay layouts (PLAN.md 9.2).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// Current line large with fill, next line below, small header.
    #[default]
    Karaoke,
    /// One line: cover, title – artist, current lyric line.
    Bar,
    /// Previous, current and next lines; the list scrolls softly.
    Stack,
    /// Only the active line, without a plate, with outline and shadow.
    Minimal,
}

impl Layout {
    pub const ALL: [Layout; 4] = [Layout::Karaoke, Layout::Bar, Layout::Stack, Layout::Minimal];

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&l| l == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// One of nine reference points of a monitor, and the matching point of the overlay.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Anchor {
    #[serde(rename = "tl")]
    TopLeft,
    #[default]
    #[serde(rename = "tc")]
    TopCenter,
    #[serde(rename = "tr")]
    TopRight,
    #[serde(rename = "ml")]
    MiddleLeft,
    #[serde(rename = "mc")]
    MiddleCenter,
    #[serde(rename = "mr")]
    MiddleRight,
    #[serde(rename = "bl")]
    BottomLeft,
    #[serde(rename = "bc")]
    BottomCenter,
    #[serde(rename = "br")]
    BottomRight,
}

impl Anchor {
    const GRID: [[Anchor; 3]; 3] = [
        [Anchor::TopLeft, Anchor::TopCenter, Anchor::TopRight],
        [Anchor::MiddleLeft, Anchor::MiddleCenter, Anchor::MiddleRight],
        [Anchor::BottomLeft, Anchor::BottomCenter, Anchor::BottomRight],
    ];

    /// Column and row in the 3×3 grid, each 0 (left/top), 1 (centre) or 2 (right/bottom).
    pub fn cell(self) -> (usize, usize) {
        for (row, cells) in Self::GRID.iter().enumerate() {
            if let Some(col) = cells.iter().position(|&a| a == self) {
                return (col, row);
            }
        }
        unreachable!("every anchor is in the grid")
    }

    pub fn from_cell(col: usize, row: usize) -> Self {
        Self::GRID[row.min(2)][col.min(2)]
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct OverlaySettings {
    pub layout: Layout,
    /// Windows device name of the monitor, e.g. `\\.\DISPLAY1`. `None` or an unknown name
    /// means the primary monitor.
    pub monitor_id: Option<String>,
    pub anchor: Anchor,
    /// Shift of the overlay's anchor point from the monitor's anchor point, in percent of the
    /// monitor width or height. Relative values survive resolution changes (PLAN.md 6.4).
    pub offset_x_pct: f64,
    pub offset_y_pct: f64,
    /// Overlay width in percent of the monitor width.
    pub width_pct: f64,
    /// Text and spacing size. 1.0 is the design size on a screen 1080 logical pixels high;
    /// larger screens scale up by the same ratio.
    pub scale: f64,
    /// Fade the overlay out this many seconds into a pause; 0 keeps it visible.
    pub hide_after_pause_s: u32,
    /// Fields this version does not know (a newer build, another branch): kept and written back.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

pub const SCALE_MIN: f64 = 0.5;
pub const SCALE_MAX: f64 = 3.0;
pub const WIDTH_PCT_MIN: f64 = 10.0;
pub const WIDTH_PCT_MAX: f64 = 100.0;
pub const HIDE_AFTER_PAUSE_MAX_S: u32 = 3600;

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            layout: Layout::Karaoke,
            monitor_id: None,
            anchor: Anchor::TopCenter,
            offset_x_pct: 0.0,
            // About 24 px below the top edge on a 1080 px screen.
            offset_y_pct: 2.2,
            width_pct: 35.0,
            scale: 1.0,
            hide_after_pause_s: 10,
            extra: Map::new(),
        }
    }
}

/// Themes (PLAN.md 9.5).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Dark translucent plate, white text, accent from the cover.
    #[default]
    GlassDark,
    /// Light translucent plate, dark text.
    GlassLight,
    /// No plate, outlined text, strong accent glow; for dark games.
    Neon,
    /// High contrast black and white, no accent.
    Mono,
    /// Plate, colours, outline and radius from `CustomTheme`.
    Custom,
}

/// Where the fill colour comes from.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccentMode {
    /// Picked from the album cover, with a contrast check (overlay/accent.ts).
    #[default]
    Cover,
    /// `accent_color`.
    Fixed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct CustomTheme {
    pub plate: bool,
    /// `#rrggbb`; the opacity is `plate_opacity`.
    pub plate_color: String,
    pub text_color: String,
    pub outline: bool,
    pub radius_px: u32,
}

impl Default for CustomTheme {
    fn default() -> Self {
        Self { plate: true, plate_color: "#0c0c10".into(), text_color: "#ffffff".into(), outline: false, radius_px: 14 }
    }
}

pub const RADIUS_MAX_PX: u32 = 32;

/// Look of the overlay (PLAN.md 9.5).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: Theme,
    /// Opacity of the plate behind the text, 0..1 (Glass Dark: 0.55).
    pub plate_opacity: f64,
    pub accent: AccentMode,
    /// `#rrggbb`, used with `AccentMode::Fixed` and as the custom theme's accent.
    pub accent_color: String,
    /// Font family for the lyrics; empty means the bundled Inter.
    pub font: String,
    /// All transitions at most 80 ms and no word glow (PLAN.md 9.4). The system setting
    /// "show animations" off has the same effect.
    pub reduced_motion: bool,
    pub custom: CustomTheme,
    /// Show lines with kana or kanji in romaji, modified Hepburn (docs/romaji.md).
    pub romaji: bool,
    /// Fields this version does not know (a newer build, another branch): kept and written back.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: Theme::GlassDark,
            plate_opacity: 0.55,
            accent: AccentMode::Cover,
            accent_color: DEFAULT_ACCENT.into(),
            font: String::new(),
            reduced_motion: false,
            custom: CustomTheme::default(),
            romaji: false,
            extra: Map::new(),
        }
    }
}

/// Sync offsets (PLAN.md 5.2, 5.7). Device and song offsets follow in Phase 3.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct SyncSettings {
    /// Positive shows the lyrics later ("lyrics too early → increase").
    pub global_offset_ms: i32,
    /// Fields this version does not know (a newer build, another branch): kept and written back.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

pub const OFFSET_LIMIT_MS: i32 = 5000;

/// Warm yellow of the fill since Phase 2; also the fallback when a cover has no usable colour.
pub const DEFAULT_ACCENT: &str = "#ffd36e";

/// `#rrggbb` (lower case) or `None`.
fn hex_color(s: &str) -> Option<String> {
    let h = s.trim().strip_prefix('#')?;
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| format!("#{}", h.to_ascii_lowercase()))
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct BehaviorSettings {
    /// Start with Windows (minimised to the tray, PLAN.md 2).
    pub autostart: bool,
    /// Do not open the settings window on a manual start either.
    pub start_minimized: bool,
    /// Fields this version does not know (a newer build, another branch): kept and written back.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Settings {
    /// Clamps every value into its valid range. Runs after loading and after each change.
    pub fn sanitize(&mut self) {
        self.schema_version = SCHEMA_VERSION;
        let o = &mut self.overlay;
        let finite_or = |v: f64, d: f64| if v.is_finite() { v } else { d };
        let d = OverlaySettings::default();
        o.offset_x_pct = finite_or(o.offset_x_pct, d.offset_x_pct).clamp(-100.0, 100.0);
        o.offset_y_pct = finite_or(o.offset_y_pct, d.offset_y_pct).clamp(-100.0, 100.0);
        o.width_pct = finite_or(o.width_pct, d.width_pct).clamp(WIDTH_PCT_MIN, WIDTH_PCT_MAX);
        o.scale = finite_or(o.scale, d.scale).clamp(SCALE_MIN, SCALE_MAX);
        o.hide_after_pause_s = o.hide_after_pause_s.min(HIDE_AFTER_PAUSE_MAX_S);
        self.sync.global_offset_ms = self.sync.global_offset_ms.clamp(-OFFSET_LIMIT_MS, OFFSET_LIMIT_MS);
        let a = &mut self.appearance;
        a.plate_opacity = finite_or(a.plate_opacity, AppearanceSettings::default().plate_opacity).clamp(0.0, 1.0);
        let dc = CustomTheme::default();
        a.accent_color = hex_color(&a.accent_color).unwrap_or_else(|| DEFAULT_ACCENT.into());
        a.custom.plate_color = hex_color(&a.custom.plate_color).unwrap_or(dc.plate_color);
        a.custom.text_color = hex_color(&a.custom.text_color).unwrap_or(dc.text_color);
        a.custom.radius_px = a.custom.radius_px.min(RADIUS_MAX_PX);
        // The name goes into a CSS font-family string: no quotes, braces or statement ends.
        a.font = a.font.trim().chars().filter(|c| !matches!(c, '"' | '\'' | ';' | '{' | '}' | '\\' | '<' | '>')).take(80).collect();
        if o.monitor_id.as_deref().is_some_and(|m| m.trim().is_empty()) {
            o.monitor_id = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_cells_roundtrip() {
        for row in 0..3 {
            for col in 0..3 {
                assert_eq!(Anchor::from_cell(col, row).cell(), (col, row));
            }
        }
        assert_eq!(serde_json::to_string(&Anchor::BottomRight).unwrap(), "\"br\"");
    }

    #[test]
    fn layouts_cycle_and_v1_files_default_to_karaoke() {
        assert_eq!(Layout::Karaoke.next(), Layout::Bar);
        assert_eq!(Layout::Bar.next(), Layout::Stack);
        assert_eq!(Layout::Minimal.next(), Layout::Karaoke);
        let s: Settings = serde_json::from_str(r#"{"schema_version":1,"overlay":{"anchor":"mr"}}"#).unwrap();
        assert_eq!((s.overlay.layout, s.overlay.anchor, s.sync.global_offset_ms), (Layout::Karaoke, Anchor::MiddleRight, 0));
    }

    #[test]
    fn phase2_files_keep_their_values_and_default_romaji_off() {
        let s: Settings = serde_json::from_str(r#"{"schema_version":1,"appearance":{"plate_opacity":0.8}}"#).unwrap();
        assert_eq!((s.appearance.plate_opacity, s.appearance.romaji), (0.8, false));
    }

    #[test]
    fn sanitize_clamps_out_of_range_values() {
        let mut s = Settings::default();
        s.schema_version = 0;
        s.overlay.scale = 9.0;
        s.overlay.width_pct = 1.0;
        s.overlay.offset_x_pct = f64::NAN;
        s.overlay.hide_after_pause_s = 99_999;
        s.overlay.monitor_id = Some("  ".into());
        s.sync.global_offset_ms = -99_999;
        s.appearance.plate_opacity = 1.7;
        s.appearance.accent_color = "red".into();
        s.appearance.custom.text_color = "#ABCDEF".into();
        s.appearance.custom.radius_px = 99;
        s.appearance.font = " Fira\"; color: red ".into();
        s.sanitize();
        assert_eq!(s.appearance.accent_color, DEFAULT_ACCENT);
        assert_eq!(s.appearance.custom.text_color, "#abcdef");
        assert_eq!(s.appearance.custom.radius_px, RADIUS_MAX_PX);
        assert_eq!(s.appearance.font, "Fira color: red");
        assert_eq!(s.sync.global_offset_ms, -OFFSET_LIMIT_MS);
        assert_eq!(s.appearance.plate_opacity, 1.0);
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        assert_eq!(s.overlay.scale, SCALE_MAX);
        assert_eq!(s.overlay.width_pct, WIDTH_PCT_MIN);
        assert_eq!(s.overlay.offset_x_pct, 0.0);
        assert_eq!(s.overlay.hide_after_pause_s, HIDE_AFTER_PAUSE_MAX_S);
        assert_eq!(s.overlay.monitor_id, None);
    }
}
