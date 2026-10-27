//! Overlay window control (PLAN.md 6): placement from the settings, show/hide without
//! activation, the edit mode, a style watchdog, the z-order guard (guard.rs) and diagnostics.
//!
//! Rule for the overlay window: after creation, never call Tauri setters that change window
//! flags (`show`, `hide`, `set_ignore_cursor_events`, `set_always_on_top`, ...). tao rewrites
//! the extended style on each of them and may call `ShowWindow(SW_SHOW)`. All state changes,
//! size and position included, go through `win32`.
//!
//! Size: the window is exactly as large as its content. The page lays the content out at the
//! width from the settings (wider for a song with long lines, up to `max_width_css`), measures
//! it and reports it (`report_content_size`); the backend then places the window by anchor and
//! offsets, so a wider plate grows around its anchor. In the edit mode the window covers
//! the whole monitor instead, so the page can draw the snap points and handle dragging with
//! pointer events (PLAN.md 6.2) while click-through is off.
//!
//! Locking: window event handlers run on the main thread, and `SetWindowPos` from another
//! thread waits for the main thread. Handlers therefore never lock `View` directly; they
//! spawn a thread that calls `relayout`.

pub mod guard;
pub mod selftest;
pub mod win32;
pub mod window;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use win32::Hwnd;
use window::{Monitor, Rect};

use crate::ipc::events;
use crate::settings::{Layout, OverlaySettings, SettingsState};

pub const LABEL: &str = "overlay";
const WATCHDOG_TICK: Duration = Duration::from_millis(500);
const TOPMOST_EVERY_TICKS: u32 = 4; // 2 s
/// Monitor changes are picked up by polling; stands in for WM_DISPLAYCHANGE (PLAN.md 6.4).
const MONITORS_EVERY_TICKS: u32 = 4; // 2 s
/// Content height at ui_scale 1 until the page reports a real one.
fn estimated_height(layout: Layout) -> f64 {
    match layout {
        Layout::Bar => 48.0,
        Layout::Karaoke => 112.0,
        Layout::Stack => 168.0,
        Layout::Minimal => 52.0,
    }
}
/// Bounds wait at most this long for the page to confirm an edit layout.
const LAYOUT_READY_TIMEOUT: Duration = Duration::from_millis(300);

type Bounds = (i32, i32, i32, i32);

#[derive(Default)]
pub struct OverlayState {
    hwnd: AtomicIsize,
    /// The user wants the overlay (tray, Ctrl+Alt+L).
    visible: AtomicBool,
    /// Hidden by "only with a fullscreen app" while `visible` stays as the user set it.
    auto_hidden: AtomicBool,
    click_through: AtomicBool,
    watchdog_paused: AtomicBool,
    view: Mutex<View>,
}

impl OverlayState {
    pub fn hwnd(&self) -> Hwnd {
        Hwnd(self.hwnd.load(Ordering::SeqCst))
    }
    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::SeqCst)
    }
    /// Visible as the user wants and not held back by the fullscreen rule.
    pub fn is_shown(&self) -> bool {
        self.is_visible() && !self.auto_hidden.load(Ordering::SeqCst)
    }
    pub fn is_click_through(&self) -> bool {
        self.click_through.load(Ordering::SeqCst)
    }
    pub fn is_editing(&self) -> bool {
        self.view.lock().unwrap().edit.is_some()
    }
    pub fn pause_watchdog(&self, paused: bool) {
        self.watchdog_paused.store(paused, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct View {
    /// Last size the page reported (CSS px) and the ui_scale it was rendered at.
    content: Option<(f64, f64, f64)>,
    edit: Option<EditSession>,
    /// Sample content for the self-test, so there is something to see without a song.
    preview: bool,
    show_seq: u64,
    edit_seq: u64,
    layout_seq: u64,
    monitors: Vec<Monitor>,
    last_layout: Option<OverlayLayout>,
    last_layout_kind: Option<Layout>,
    last_bounds: Option<Bounds>,
    /// Bounds waiting until the page has drawn the layout with this sequence number.
    pending_bounds: Option<(u64, Bounds)>,
}

struct EditSession {
    /// Working copy of the geometry; the settings change only when the edit mode saves.
    geometry: OverlaySettings,
    seq: u64,
}

/// What the page reports while editing: the content box in CSS px relative to the monitor.
#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub struct EditDraft {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub width_css: f64,
    pub scale: f64,
}

/// Payload of the `overlay-layout` event.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct OverlayLayout {
    pub layout: Layout,
    pub width_css: f64,
    /// The page may widen the plate up to this for long lyric lines (monitor minus edge margins).
    pub max_width_css: f64,
    pub ui_scale: f64,
    /// Content alignment from the anchor column: "start", "center" or "end".
    pub align: &'static str,
    pub preview: bool,
    /// Counts how often the overlay was shown, so the page can show a short hint.
    pub show_seq: u64,
    pub edit: Option<EditLayout>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct EditLayout {
    /// Changes when the page must (re)start its draft: edit start, monitor switch.
    pub seq: u64,
    pub monitor_width: f64,
    pub monitor_height: f64,
    pub monitor_label: String,
    pub monitor_count: usize,
    /// Content position in CSS px relative to the monitor.
    pub x: f64,
    pub y: f64,
    /// The settings value; ui_scale = scale × res_factor.
    pub scale: f64,
    pub res_factor: f64,
    pub snap_margin: f64,
}

#[derive(Serialize, Clone, Debug)]
pub struct LayoutEvent {
    pub seq: u64,
    #[serde(flatten)]
    pub layout: OverlayLayout,
}

#[derive(Serialize, Clone, Debug)]
pub struct OverlayDiagnostics {
    pub visible: bool,
    pub click_through: bool,
    pub edit_mode: bool,
    pub bounds: String,
    pub ex_style: u32,
    pub ex_style_text: String,
    pub topmost: bool,
    pub no_activate: bool,
    pub tool_window: bool,
    pub transparent: bool,
    pub layered: bool,
    pub app_window: bool,
    pub foreground_is_overlay: bool,
    pub foreground_app: String,
    pub auto_hidden: bool,
    pub guard: guard::GuardStatus,
}

pub fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Applies styles and geometry to the hidden overlay, then shows it.
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let Some(win) = window(app) else {
        tracing::error!("overlay window missing from tauri.conf.json");
        return Ok(());
    };
    let hwnd = Hwnd::from_raw(win.hwnd()?);
    let state = app.state::<OverlayState>();
    state.hwnd.store(hwnd.0, Ordering::SeqCst);
    state.click_through.store(true, Ordering::SeqCst);
    win32::apply_overlay_exstyles(hwnd, true);
    relayout(app);
    show(app);
    tracing::info!(style = %win32::describe_ex_style(win32::ex_style(hwnd)), "overlay shown");
    spawn_watchdog(app.clone());
    guard::spawn(app.clone());
    Ok(())
}

/// Recomputes size, position and page layout from the settings (or the edit draft) and the
/// current monitors. Cheap when nothing changed.
pub fn relayout(app: &AppHandle) {
    let state = app.state::<OverlayState>();
    let h = state.hwnd();
    if h.is_null() {
        return;
    }
    let settings = app.state::<SettingsState>().get().overlay;
    let mut view = state.view.lock().unwrap();
    view.monitors = window::list_monitors();
    let geometry = view.edit.as_ref().map(|e| e.geometry.clone()).unwrap_or(settings);
    let Some(m) = window::pick(&view.monitors, geometry.monitor_id.as_deref()).cloned() else {
        tracing::warn!("no monitor found, overlay not placed");
        return;
    };
    let content = window::content_layout(&geometry, &m);
    // A layout switch changes the height entirely; scaling the old height would be wrong.
    let layout_changed = view.last_layout_kind.is_some_and(|l| l != geometry.layout);
    view.last_layout_kind = Some(geometry.layout);
    if layout_changed {
        view.content = None;
    }
    let height = match view.content {
        Some((_, h, rendered_scale)) if rendered_scale > 0.0 => h / rendered_scale * content.ui_scale,
        _ => estimated_height(geometry.layout) * content.ui_scale,
    };
    let (monitor_width, monitor_height) = m.css_size();
    let max_width_css = (monitor_width - 2.0 * m.edge_margin_css()).max(content.width_css);
    // The page widens the plate for songs with long lines; the window follows its report.
    let width = match (&view.edit, view.content) {
        (None, Some((w, _, _))) if w > content.width_css + 0.5 => w.min(max_width_css),
        _ => content.width_css,
    };
    let rect = window::place(&geometry, &m, (width, height));
    let edit = view.edit.as_ref().map(|e| EditLayout {
        seq: e.seq,
        monitor_width,
        monitor_height,
        monitor_label: m.label(),
        monitor_count: view.monitors.len(),
        x: rect.x,
        y: rect.y,
        scale: geometry.scale,
        res_factor: m.resolution_factor(),
        snap_margin: m.edge_margin_css(),
    });
    let bounds = if edit.is_some() { (m.x, m.y, m.width, m.height) } else { window::to_physical(&m, &rect) };
    let layout = OverlayLayout {
        layout: geometry.layout,
        width_css: content.width_css,
        max_width_css,
        ui_scale: content.ui_scale,
        align: window::align(geometry.anchor),
        preview: view.preview,
        show_seq: view.show_seq,
        edit,
    };

    let edit_seq = |l: &OverlayLayout| l.edit.as_ref().map(|e| e.seq);
    let mut wait_for = None;
    if view.last_layout.as_ref() != Some(&layout) {
        // Entering the edit mode or switching its monitor: grow the window only after the page
        // moved the content to its place, else the content flashes in the corner for a frame.
        if layout.edit.is_some() && view.last_layout.as_ref().map(edit_seq) != Some(edit_seq(&layout)) {
            wait_for = Some(view.layout_seq + 1);
        }
        view.layout_seq += 1;
        let event = LayoutEvent { seq: view.layout_seq, layout: layout.clone() };
        if let Err(e) = app.emit(events::OVERLAY_LAYOUT, &event) {
            tracing::warn!(error = %e, "could not emit overlay layout");
        }
        view.last_layout = Some(layout);
    }
    if let Some(seq) = wait_for {
        view.pending_bounds = Some((seq, bounds));
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(LAYOUT_READY_TIMEOUT);
            layout_ready(&app, seq);
        });
    } else if let Some((seq, _)) = view.pending_bounds {
        view.pending_bounds = Some((seq, bounds));
    } else {
        apply_bounds(&mut view, h, bounds);
    }
}

/// Like `relayout`, but re-applies the bounds even if they look unchanged (after tao or
/// Windows moved the window, e.g. on a DPI change).
pub fn force_relayout(app: &AppHandle) {
    app.state::<OverlayState>().view.lock().unwrap().last_bounds = None;
    relayout(app);
}

fn apply_bounds(view: &mut View, h: Hwnd, b: Bounds) {
    if view.last_bounds != Some(b) {
        win32::set_bounds(h, b.0, b.1, b.2, b.3);
        view.last_bounds = Some(b);
        tracing::debug!(x = b.0, y = b.1, width = b.2, height = b.3, "overlay bounds");
    }
}

/// The page has drawn layout `seq`; pending bounds can now be applied.
pub fn layout_ready(app: &AppHandle, seq: u64) {
    let state = app.state::<OverlayState>();
    let h = state.hwnd();
    let mut view = state.view.lock().unwrap();
    if let Some((wait, bounds)) = view.pending_bounds
        && seq >= wait
    {
        view.pending_bounds = None;
        apply_bounds(&mut view, h, bounds);
    }
}

pub fn current_layout(app: &AppHandle) -> Option<LayoutEvent> {
    let state = app.state::<OverlayState>();
    let view = state.view.lock().unwrap();
    view.last_layout.clone().map(|layout| LayoutEvent { seq: view.layout_seq, layout })
}

/// The page measured its content (CSS px) at `ui_scale`.
pub fn report_content_size(app: &AppHandle, width: f64, height: f64, ui_scale: f64) {
    if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
        return;
    }
    let state = app.state::<OverlayState>();
    let relayout_needed = {
        let mut view = state.view.lock().unwrap();
        let next = Some((width, height, ui_scale));
        let changed = view.content != next;
        view.content = next;
        changed && view.edit.is_none()
    };
    if relayout_needed {
        relayout(app);
    }
}

pub fn show(app: &AppHandle) {
    let state = app.state::<OverlayState>();
    let h = state.hwnd();
    if h.is_null() {
        return;
    }
    state.visible.store(true, Ordering::SeqCst);
    if !state.auto_hidden.load(Ordering::SeqCst) || state.is_editing() {
        win32::show_no_activate(h);
        win32::reassert_topmost(h, false);
    }
    state.view.lock().unwrap().show_seq += 1;
    relayout(app);
    crate::tray::sync_overlay_visible(app, true);
}

pub fn hide(app: &AppHandle) {
    let state = app.state::<OverlayState>();
    let h = state.hwnd();
    if h.is_null() {
        return;
    }
    if state.is_editing() {
        cancel_edit(app);
    }
    win32::hide(h);
    state.visible.store(false, Ordering::SeqCst);
    crate::tray::sync_overlay_visible(app, false);
}

/// Toggles visibility; returns the new state.
pub fn toggle(app: &AppHandle) -> bool {
    let visible = app.state::<OverlayState>().is_visible();
    if visible {
        hide(app)
    } else {
        show(app)
    }
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

pub fn set_preview(app: &AppHandle, preview: bool) {
    app.state::<OverlayState>().view.lock().unwrap().preview = preview;
    relayout(app);
}

/// "Only with a fullscreen app" (guard.rs): hides or shows the window without changing what
/// the user chose. The edit mode and the settings preview always stay visible.
pub fn set_auto_hidden(app: &AppHandle, hidden: bool) {
    let state = app.state::<OverlayState>();
    let h = state.hwnd();
    let hidden = hidden && !state.is_editing() && !state.view.lock().unwrap().preview;
    if h.is_null() || state.auto_hidden.swap(hidden, Ordering::SeqCst) == hidden {
        return;
    }
    tracing::info!(hidden, "overlay hidden or shown by the fullscreen rule");
    if !state.is_visible() {
        return;
    }
    if hidden {
        win32::hide(h);
    } else {
        win32::show_no_activate(h);
        win32::reassert_topmost(h, false);
        relayout(app);
    }
}

// ---- Edit mode (PLAN.md 9.6) ----

/// Starts the edit mode: overlay over the whole monitor, click-through off, Enter/Esc
/// registered. The overlay keeps WS_EX_NOACTIVATE, so the game keeps the keyboard focus.
pub fn enter_edit(app: &AppHandle) {
    let state = app.state::<OverlayState>();
    if state.hwnd().is_null() {
        return;
    }
    {
        let mut view = state.view.lock().unwrap();
        if view.edit.is_some() {
            return;
        }
        view.edit_seq += 1;
        let seq = view.edit_seq;
        view.edit = Some(EditSession { geometry: app.state::<SettingsState>().get().overlay, seq });
    }
    set_click_through(app, false);
    if state.is_shown() {
        relayout(app);
    } else {
        show(app);
        // Held back by the fullscreen rule: the edit mode needs the window anyway.
        win32::show_no_activate(state.hwnd());
        win32::reassert_topmost(state.hwnd(), false);
    }
    crate::hotkeys::set_edit_keys(app, true);
    crate::tray::sync_edit_mode(app, true);
    tracing::info!("edit mode started");
}

/// Takes over what the page reports after each drag or resize.
pub fn edit_update(app: &AppHandle, draft: EditDraft) {
    let state = app.state::<OverlayState>();
    let mut view = state.view.lock().unwrap();
    let View { edit, monitors, content, .. } = &mut *view;
    let Some(edit) = edit.as_mut() else { return };
    let Some(m) = window::pick(monitors, edit.geometry.monitor_id.as_deref()) else { return };
    let rect = Rect { x: draft.x, y: draft.y, width: draft.width, height: draft.height };
    let (anchor, offset_x, offset_y, width_pct) = window::geometry_from_rect(m, &rect, draft.width_css);
    let g = &mut edit.geometry;
    g.anchor = anchor;
    g.offset_x_pct = offset_x;
    g.offset_y_pct = offset_y;
    g.width_pct = width_pct;
    g.scale = draft.scale;
    *content = Some((draft.width, draft.height, draft.scale * m.resolution_factor()));
}

/// Moves the edit session to the next (`step` = 1) or previous monitor, keeping the relative position.
pub fn edit_switch_monitor(app: &AppHandle, step: i32) {
    let state = app.state::<OverlayState>();
    {
        let mut view = state.view.lock().unwrap();
        view.monitors = window::list_monitors();
        let count = view.monitors.len();
        if count < 2 {
            return;
        }
        let View { edit, monitors, edit_seq, .. } = &mut *view;
        let Some(edit) = edit.as_mut() else { return };
        let current = window::pick(monitors, edit.geometry.monitor_id.as_deref())
            .and_then(|m| monitors.iter().position(|x| x.id == m.id))
            .unwrap_or(0);
        let next = (current as i32 + step).rem_euclid(count as i32) as usize;
        edit.geometry.monitor_id = Some(monitors[next].id.clone());
        *edit_seq += 1;
        edit.seq = *edit_seq;
        tracing::info!(monitor = %monitors[next].id, "edit mode switched monitor");
    }
    relayout(app);
}

/// Saves the edit session into the settings and leaves the edit mode. `draft` is the page's
/// latest state when the save comes from the page itself.
pub fn save_edit(app: &AppHandle, draft: Option<EditDraft>) {
    if let Some(d) = draft {
        edit_update(app, d);
    }
    let session = app.state::<OverlayState>().view.lock().unwrap().edit.take();
    let Some(session) = session else { return };
    let g = session.geometry;
    let (before, after) = app.state::<SettingsState>().update(|s| {
        let o = &mut s.overlay;
        o.monitor_id = g.monitor_id.clone();
        o.anchor = g.anchor;
        o.offset_x_pct = g.offset_x_pct;
        o.offset_y_pct = g.offset_y_pct;
        o.width_pct = g.width_pct;
        o.scale = g.scale;
    });
    tracing::info!(
        anchor = ?after.overlay.anchor,
        offset_x_pct = after.overlay.offset_x_pct,
        offset_y_pct = after.overlay.offset_y_pct,
        width_pct = after.overlay.width_pct,
        scale = after.overlay.scale,
        monitor = after.overlay.monitor_id.as_deref().unwrap_or("primary"),
        "edit mode saved"
    );
    finish_edit(app);
    crate::ipc::settings_changed(app, &before, &after);
}

pub fn cancel_edit(app: &AppHandle) {
    let session = app.state::<OverlayState>().view.lock().unwrap().edit.take();
    if session.is_some() {
        tracing::info!("edit mode cancelled");
        finish_edit(app);
    }
}

fn finish_edit(app: &AppHandle) {
    set_click_through(app, true);
    relayout(app);
    crate::hotkeys::set_edit_keys(app, false);
    crate::tray::sync_edit_mode(app, false);
}

// ---- Diagnostics and watchdog ----

pub fn diagnostics(app: &AppHandle) -> OverlayDiagnostics {
    let state = app.state::<OverlayState>();
    let h = state.hwnd();
    let s = win32::ex_style(h);
    let fg = win32::foreground();
    let bounds = win32::window_rect(h)
        .map(|r| format!("{}×{} bei ({}, {})", r.right - r.left, r.bottom - r.top, r.left, r.top))
        .unwrap_or_default();
    OverlayDiagnostics {
        visible: win32::is_visible(h),
        click_through: state.is_click_through(),
        edit_mode: state.is_editing(),
        bounds,
        ex_style: s.0,
        ex_style_text: win32::describe_ex_style(s),
        topmost: win32::has(s, win32::EX_TOPMOST),
        no_activate: win32::has(s, win32::EX_NOACTIVATE),
        tool_window: win32::has(s, win32::EX_TOOLWINDOW),
        transparent: win32::has(s, win32::EX_TRANSPARENT),
        layered: win32::has(s, win32::EX_LAYERED),
        app_window: win32::has(s, win32::EX_APPWINDOW),
        foreground_is_overlay: fg == h && !h.is_null(),
        foreground_app: win32::window_program(fg),
        auto_hidden: state.auto_hidden.load(Ordering::SeqCst),
        guard: guard::status(),
    }
}

/// Restores lost style bits, visibility, topmost order and bounds (PLAN.md 6.1, 6.3, 6.4).
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
                // Hidden by the user or by the fullscreen rule (the edit mode is never held back).
                if h.is_null() || !(state.is_shown() || state.is_editing()) {
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
                if tick.is_multiple_of(TOPMOST_EVERY_TICKS) {
                    win32::reassert_topmost(h, false);
                }
                if tick.is_multiple_of(MONITORS_EVERY_TICKS) {
                    check_geometry(&app, h);
                }
            }
        })
        .expect("failed to spawn overlay watchdog");
}

/// Relayouts when the monitor setup changed or something else moved the window.
fn check_geometry(app: &AppHandle, h: Hwnd) {
    let monitors = window::list_monitors();
    let state = app.state::<OverlayState>();
    let (monitors_changed, drifted) = {
        let view = state.view.lock().unwrap();
        let drifted = view.pending_bounds.is_none()
            && match (view.last_bounds, win32::window_rect(h)) {
                (Some(b), Some(r)) => (r.left, r.top, r.right - r.left, r.bottom - r.top) != b,
                _ => false,
            };
        (view.monitors != monitors, drifted)
    };
    if monitors_changed {
        tracing::info!(count = monitors.len(), "monitor setup changed, re-placing overlay");
        force_relayout(app);
    } else if drifted {
        tracing::debug!("overlay bounds drifted, re-applying");
        force_relayout(app);
    }
}
