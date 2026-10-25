//! Overlay window control (PLAN.md 6). Phase 0 scope: placement, show/hide without
//! activation, click-through, a style watchdog and diagnostics for Spike B.
//!
//! Rule for the overlay window: after creation, never call Tauri setters that change
//! window flags (`show`, `hide`, `set_ignore_cursor_events`, `set_always_on_top`, ...).
//! tao rewrites the extended style on each of them and may call `ShowWindow(SW_SHOW)`.
//! All state changes go through `win32`.

pub mod selftest;
pub mod win32;

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, WebviewWindow};
use win32::Hwnd;

pub const LABEL: &str = "overlay";
const WIDTH: f64 = 900.0;
const HEIGHT: f64 = 170.0;
const TOP_MARGIN: f64 = 24.0;
const WATCHDOG_TICK: Duration = Duration::from_millis(500);
const TOPMOST_EVERY_TICKS: u32 = 4; // 2 s

#[derive(Default)]
pub struct OverlayState {
    hwnd: AtomicIsize,
    visible: AtomicBool,
    click_through: AtomicBool,
    watchdog_paused: AtomicBool,
}

impl OverlayState {
    pub fn hwnd(&self) -> Hwnd {
        Hwnd(self.hwnd.load(Ordering::SeqCst))
    }
    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::SeqCst)
    }
    pub fn is_click_through(&self) -> bool {
        self.click_through.load(Ordering::SeqCst)
    }
    pub fn pause_watchdog(&self, paused: bool) {
        self.watchdog_paused.store(paused, Ordering::SeqCst);
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct OverlayDiagnostics {
    pub visible: bool,
    pub click_through: bool,
    pub ex_style: u32,
    pub ex_style_text: String,
    pub topmost: bool,
    pub no_activate: bool,
    pub tool_window: bool,
    pub transparent: bool,
    pub layered: bool,
    pub app_window: bool,
    pub foreground_is_overlay: bool,
    pub foreground_title: String,
}

pub fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Places the overlay top-centre on the primary monitor, applies its styles and shows it.
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let Some(win) = window(app) else {
        tracing::error!("overlay window missing from tauri.conf.json");
        return Ok(());
    };

    // Geometry first, while the window is still hidden. set_size/set_position do not
    // change tao window flags, so they are safe even later.
    if let Some(monitor) = win.primary_monitor()? {
        let scale = monitor.scale_factor();
        let width_px = (WIDTH * scale).round() as i32;
        let x = monitor.position().x + (monitor.size().width as i32 - width_px) / 2;
        let y = monitor.position().y + (TOP_MARGIN * scale).round() as i32;
        win.set_size(LogicalSize::new(WIDTH, HEIGHT))?;
        win.set_position(PhysicalPosition::new(x, y))?;
        tracing::info!(x, y, scale, "overlay placed top-centre on primary monitor");
    }

    let hwnd = Hwnd::from_raw(win.hwnd()?);
    let state = app.state::<OverlayState>();
    state.hwnd.store(hwnd.0, Ordering::SeqCst);
    state.click_through.store(true, Ordering::SeqCst);
    win32::apply_overlay_exstyles(hwnd, true);
    show(app);
    tracing::info!(style = %win32::describe_ex_style(win32::ex_style(hwnd)), "overlay shown");

    spawn_watchdog(app.clone());
    Ok(())
}

pub fn show(app: &AppHandle) {
    let state = app.state::<OverlayState>();
    let h = state.hwnd();
    if h.is_null() {
        return;
    }
    win32::show_no_activate(h);
    win32::reassert_topmost(h, false);
    state.visible.store(true, Ordering::SeqCst);
}

pub fn hide(app: &AppHandle) {
    let state = app.state::<OverlayState>();
    let h = state.hwnd();
    if h.is_null() {
        return;
    }
    win32::hide(h);
    state.visible.store(false, Ordering::SeqCst);
}

/// Toggles visibility; returns the new state.
pub fn toggle(app: &AppHandle) -> bool {
    let visible = app.state::<OverlayState>().is_visible();
    if visible { hide(app) } else { show(app) }
    tracing::info!(visible = !visible, "overlay toggled");
    !visible
}

pub fn set_click_through(app: &AppHandle, enabled: bool) {
    let state = app.state::<OverlayState>();
    let h = state.hwnd();
    if h.is_null() {
        return;
    }
    state.click_through.store(enabled, Ordering::SeqCst);
    win32::apply_overlay_exstyles(h, enabled);
    tracing::info!(enabled, style = %win32::describe_ex_style(win32::ex_style(h)), "overlay click-through set");
}

pub fn toggle_click_through(app: &AppHandle) -> bool {
    let enabled = !app.state::<OverlayState>().is_click_through();
    set_click_through(app, enabled);
    enabled
}

pub fn diagnostics(app: &AppHandle) -> OverlayDiagnostics {
    let state = app.state::<OverlayState>();
    let h = state.hwnd();
    let s = win32::ex_style(h);
    let fg = win32::foreground();
    OverlayDiagnostics {
        visible: win32::is_visible(h),
        click_through: state.is_click_through(),
        ex_style: s.0,
        ex_style_text: win32::describe_ex_style(s),
        topmost: win32::has(s, win32::EX_TOPMOST),
        no_activate: win32::has(s, win32::EX_NOACTIVATE),
        tool_window: win32::has(s, win32::EX_TOOLWINDOW),
        transparent: win32::has(s, win32::EX_TRANSPARENT),
        layered: win32::has(s, win32::EX_LAYERED),
        app_window: win32::has(s, win32::EX_APPWINDOW),
        foreground_is_overlay: fg == h && !h.is_null(),
        foreground_title: win32::window_title(fg),
    }
}

/// Restores lost style bits, lost visibility and topmost order (PLAN.md 6.1, 6.3).
fn spawn_watchdog(app: AppHandle) {
    std::thread::Builder::new()
        .name("overlay-watchdog".into())
        .spawn(move || {
            let mut tick: u32 = 0;
            loop {
                std::thread::sleep(WATCHDOG_TICK);
                tick = tick.wrapping_add(1);
                let state = app.state::<OverlayState>();
                if state.watchdog_paused.load(Ordering::SeqCst) {
                    continue;
                }
                let h = state.hwnd();
                if h.is_null() || !state.is_visible() {
                    continue;
                }
                let click_through = state.is_click_through();
                if !win32::exstyles_ok(h, click_through) {
                    tracing::warn!(
                        style = %win32::describe_ex_style(win32::ex_style(h)),
                        "overlay lost style bits, re-applying"
                    );
                    win32::apply_overlay_exstyles(h, click_through);
                }
                if !win32::is_visible(h) {
                    tracing::warn!("overlay lost visibility, showing again without activation");
                    win32::show_no_activate(h);
                }
                if tick % TOPMOST_EVERY_TICKS == 0 {
                    win32::reassert_topmost(h, false);
                }
            }
        })
        .expect("failed to spawn overlay watchdog");
}
