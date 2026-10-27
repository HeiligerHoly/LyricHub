//! Keeps the overlay above games and reacts to the desktop (PLAN.md 6.3, 6.4, Phase 5).
//!
//! One thread with a message loop owns two out-of-context WinEvent hooks and a hidden window:
//! - `EVENT_OBJECT_REORDER` and `EVENT_SYSTEM_FOREGROUND`: when another topmost window lands
//!   above the overlay and overlaps it, the overlay goes back to the top of the topmost band.
//!   Out of context means Windows delivers the events to this thread; nothing runs inside the
//!   game. Spike B showed that re-asserting on a timer loses against games that re-assert
//!   themselves (82 % on top at 100 ms).
//! - A one-second timer classifies the foreground window (normal, borderless fullscreen,
//!   exclusive fullscreen) for "only with a fullscreen app" and notices when the overlay
//!   stays covered, which the tray then reports.
//! - The hidden window receives `WM_DISPLAYCHANGE` (re-place the overlay) and
//!   `WM_POWERBROADCAST` (after sleep the media session is connected again).
//!
//! Menus, tooltips, the taskbar and other shell surfaces are left alone, and the overlay
//! re-asserts at most `BURST_MAX` times per second, so two windows that both want the top
//! cannot spin.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, EVENT_OBJECT_REORDER, EVENT_SYSTEM_FOREGROUND, GW_HWNDNEXT,
    GWL_EXSTYLE, GetClassNameW, GetForegroundWindow, GetMessageW, GetTopWindow, GetWindow, GetWindowLongPtrW,
    GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible, MSG, PBT_APMRESUMEAUTOMATIC, RegisterClassW,
    SetTimer, TranslateMessage, WINDOW_EX_STYLE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_DISPLAYCHANGE,
    WM_POWERBROADCAST, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPED,
};
use windows::core::w;

use super::OverlayState;
use super::win32::{self, Hwnd};

/// At most this many re-asserts per second; beyond that another window fights for the top.
const BURST_MAX: u32 = 10;
const TICK: Duration = Duration::from_secs(1);
/// The overlay counts as covered once a window stayed above it this long.
const COVERED_AFTER: Duration = Duration::from_secs(2);
/// After a display change Windows moves windows around for a moment.
const DISPLAY_SETTLE: Duration = Duration::from_millis(400);
const TIMER_ID: usize = 1;

/// Shell and system surfaces that may sit above the overlay: menus, tooltips, taskbar, start
/// menu and search, Alt+Tab and task view, IME candidates.
const LEAVE_ALONE: &[&str] = &[
    "#32768",
    "tooltips_class32",
    "SysShadow",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "NotifyIconOverflowWindow",
    "TopLevelWindowForOverflowXamlIsland",
    "Xaml_WindowedPopupClass",
    "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
    "MultitaskingViewFrame",
    "ForegroundStaging",
    "IME",
    "MSCTFIME UI",
];
/// Desktop and shell windows are never a fullscreen app.
const SHELL: &[&str] = &["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

/// What runs in the foreground (for "only with a fullscreen app" and the diagnostics).
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    #[default]
    Normal,
    /// A window that covers its whole monitor: borderless fullscreen, video, game.
    Borderless,
    /// Direct3D exclusive fullscreen. Windows usually still composes the overlay on top
    /// (fullscreen optimisations), but not always.
    Exclusive,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct GuardStatus {
    pub screen: Screen,
    /// A window stayed above the overlay despite re-asserting.
    pub covered: bool,
    pub covered_by: Option<String>,
    pub reasserts: u32,
}

static APP: OnceLock<AppHandle> = OnceLock::new();
static SCREEN: AtomicU8 = AtomicU8::new(0);
static REASSERTS: AtomicU32 = AtomicU32::new(0);
/// Title (or class) of the window that keeps covering the overlay; read by `status()`.
static COVERED_BY: Mutex<Option<String>> = Mutex::new(None);

#[derive(Default)]
struct Local {
    burst_start: Option<Instant>,
    burst: u32,
    contest_logged_at: Option<Instant>,
    above_since: Option<Instant>,
    covered: bool,
}

thread_local! {
    static LOCAL: RefCell<Local> = RefCell::new(Local::default());
}

pub fn status() -> GuardStatus {
    let covered_by = COVERED_BY.lock().unwrap().clone();
    GuardStatus {
        screen: screen(),
        covered: covered_by.is_some(),
        covered_by,
        reasserts: REASSERTS.load(Ordering::Relaxed),
    }
}

pub fn screen() -> Screen {
    match SCREEN.load(Ordering::Relaxed) {
        1 => Screen::Borderless,
        2 => Screen::Exclusive,
        _ => Screen::Normal,
    }
}

pub fn spawn(app: AppHandle) {
    if APP.set(app).is_err() {
        return;
    }
    std::thread::Builder::new().name("overlay-guard".into()).spawn(run).expect("failed to spawn overlay guard");
}

fn run() {
    // SAFETY: hooks and window belong to this thread, which pumps messages until the app ends.
    unsafe {
        let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        let reorder: HWINEVENTHOOK =
            SetWinEventHook(EVENT_OBJECT_REORDER, EVENT_OBJECT_REORDER, None, Some(on_event), 0, 0, flags);
        let foreground: HWINEVENTHOOK =
            SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(on_event), 0, 0, flags);
        if reorder.is_invalid() || foreground.is_invalid() {
            tracing::warn!("WinEvent hooks unavailable, the watchdog alone keeps the overlay on top");
        }
        let window = create_message_window();
        if let Some(w) = window {
            SetTimer(Some(w), TIMER_ID, TICK.as_millis() as u32, None);
        }
        tracing::info!(
            hooks = !reorder.is_invalid() && !foreground.is_invalid(),
            window = window.is_some(),
            "overlay guard started"
        );
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe fn create_message_window() -> Option<HWND> {
    // SAFETY: registers and creates a hidden top-level window owned by this thread. A
    // message-only window would not get the WM_DISPLAYCHANGE and WM_POWERBROADCAST broadcasts.
    unsafe {
        let instance = GetModuleHandleW(None).ok()?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: w!("LyricHubGuard"),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return None;
        }
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            w!("LyricHubGuard"),
            w!("LyricHub Guard"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .ok()
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER if wparam.0 == TIMER_ID => {
            tick();
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            if let Some(app) = APP.get().cloned() {
                tracing::info!(width = lparam.0 & 0xffff, height = (lparam.0 >> 16) & 0xffff, "display changed");
                std::thread::spawn(move || {
                    std::thread::sleep(DISPLAY_SETTLE);
                    super::force_relayout(&app);
                });
            }
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            if wparam.0 as u32 == PBT_APMRESUMEAUTOMATIC {
                tracing::info!("resumed from sleep, reconnecting the media session");
                crate::sources::smtc::request_reconnect();
                if let Some(app) = APP.get().cloned() {
                    std::thread::spawn(move || {
                        std::thread::sleep(DISPLAY_SETTLE);
                        super::force_relayout(&app);
                    });
                }
            }
            // SAFETY: default handling for everything else of this message.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        // SAFETY: default handling for our own window.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

unsafe extern "system" fn on_event(_: HWINEVENTHOOK, event: u32, _hwnd: HWND, _: i32, _: i32, _: u32, _: u32) {
    if event == EVENT_SYSTEM_FOREGROUND {
        classify();
    }
    defend();
}

/// The overlay window if it should be on screen right now.
fn overlay() -> Option<(AppHandle, Hwnd)> {
    let app = APP.get()?;
    let state = app.state::<OverlayState>();
    let h = state.hwnd();
    (!h.is_null() && state.is_visible() && win32::is_visible(h)).then(|| (app.clone(), h))
}

/// Puts the overlay back on top when another window covers it, within the burst limit.
fn defend() {
    let Some((_, h)) = overlay() else { return };
    let Some(cover) = covering_window(h) else {
        LOCAL.with(|l| l.borrow_mut().above_since = None);
        return;
    };
    let now = Instant::now();
    let allowed = LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        if l.burst_start.is_none_or(|s| now.duration_since(s) >= Duration::from_secs(1)) {
            l.burst_start = Some(now);
            l.burst = 0;
        }
        l.burst += 1;
        if l.burst > BURST_MAX && l.contest_logged_at.is_none_or(|t| now.duration_since(t) > Duration::from_secs(60)) {
            l.contest_logged_at = Some(now);
            tracing::warn!(window = %describe(cover), "another window keeps taking the top, re-asserting at most {BURST_MAX} times per second");
        }
        if l.above_since.is_none() {
            l.above_since = Some(now);
        }
        l.burst <= BURST_MAX
    });
    if allowed {
        win32::reassert_topmost(h, false);
        REASSERTS.fetch_add(1, Ordering::Relaxed);
    }
    if covering_window(h).is_none() {
        LOCAL.with(|l| l.borrow_mut().above_since = None);
    }
}

/// Once per second: fullscreen state, the "only with a fullscreen app" rule and the covered
/// hint.
fn tick() {
    classify();
    defend();
    let Some(app) = APP.get() else { return };
    let (changed, by) = LOCAL.with(|l| {
        let mut l = l.borrow_mut();
        let covered = l.above_since.is_some_and(|t| t.elapsed() >= COVERED_AFTER);
        let by = if covered { overlay().and_then(|(_, h)| covering_window(h)).map(describe) } else { None };
        let changed = covered != l.covered;
        l.covered = covered;
        (changed, by)
    });
    *COVERED_BY.lock().unwrap() = by.clone();
    if changed {
        match &by {
            Some(w) => tracing::warn!(window = %w, "overlay covered by another window"),
            None => tracing::info!("overlay on top again"),
        }
    }
    crate::tray::set_hint(app, hint(app, by.as_deref()));
}

/// Tray hint: the overlay stays covered, or a game runs in exclusive fullscreen, where Windows
/// may not compose the overlay at all.
fn hint(app: &AppHandle, covered_by: Option<&str>) -> Option<String> {
    use crate::i18n::Lang;
    let lang = crate::i18n::current(app);
    if let Some(w) = covered_by {
        return Some(match lang {
            Lang::De => format!("⚠ Overlay verdeckt von „{w}“: im Spiel „Randloses Fenster“ wählen"),
            Lang::En => format!("⚠ Overlay covered by “{w}”: switch the game to borderless window"),
        });
    }
    (screen() == Screen::Exclusive && overlay().is_some()).then(|| {
        crate::i18n::pick(
            lang,
            "Spiel im exklusiven Vollbild: Fehlt das Overlay, im Spiel „Randloses Fenster“ wählen",
            "Game in exclusive fullscreen: if the overlay is missing, choose borderless window in the game",
        )
        .to_owned()
    })
}

/// Classifies the foreground and applies "only with a fullscreen app".
fn classify() {
    let now = foreground_screen();
    let code = match now {
        Screen::Normal => 0,
        Screen::Borderless => 1,
        Screen::Exclusive => 2,
    };
    let before = SCREEN.swap(code, Ordering::Relaxed);
    let Some(app) = APP.get() else { return };
    if before != code {
        tracing::debug!(screen = ?now, "foreground screen mode");
    }
    let only_fullscreen = app.state::<crate::settings::SettingsState>().get().overlay.only_fullscreen;
    super::set_auto_hidden(app, only_fullscreen && now == Screen::Normal);
}

fn foreground_screen() -> Screen {
    // SAFETY: plain queries about the foreground window and its monitor.
    unsafe {
        if SHQueryUserNotificationState().is_ok_and(|s| s == QUNS_RUNNING_D3D_FULL_SCREEN) {
            return Screen::Exclusive;
        }
        let fg = GetForegroundWindow();
        if fg.is_invalid() || own_process(fg) || SHELL.contains(&class_name(fg).as_str()) {
            return Screen::Normal;
        }
        let mut r = RECT::default();
        if GetWindowRect(fg, &mut r).is_err() {
            return Screen::Normal;
        }
        let monitor = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return Screen::Normal;
        }
        let m = info.rcMonitor;
        if r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom {
            Screen::Borderless
        } else {
            Screen::Normal
        }
    }
}

/// The first window above the overlay that overlaps it and is not a shell surface.
fn covering_window(overlay: Hwnd) -> Option<HWND> {
    let rect = win32::window_rect(overlay)?;
    // SAFETY: walks the z-order from the top until the overlay; each call only reads.
    unsafe {
        let mut cur = GetTopWindow(None).ok()?;
        for _ in 0..1024 {
            if cur == overlay.raw() {
                return None;
            }
            if covers(cur, &rect) {
                return Some(cur);
            }
            cur = GetWindow(cur, GW_HWNDNEXT).ok()?;
        }
    }
    None
}

unsafe fn covers(h: HWND, overlay: &RECT) -> bool {
    // SAFETY: read-only queries on a window handle from the z-order walk.
    unsafe {
        let ex = WINDOW_EX_STYLE(GetWindowLongPtrW(h, GWL_EXSTYLE) as u32);
        if ex.0 & WS_EX_TOPMOST.0 == 0 || !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() || own_process(h) {
            return false;
        }
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(h, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), size_of::<u32>() as u32);
        if cloaked != 0 || LEAVE_ALONE.contains(&class_name(h).as_str()) {
            return false;
        }
        let mut r = RECT::default();
        GetWindowRect(h, &mut r).is_ok()
            && r.left < overlay.right
            && r.right > overlay.left
            && r.top < overlay.bottom
            && r.bottom > overlay.top
    }
}

fn own_process(h: HWND) -> bool {
    let mut pid = 0u32;
    // SAFETY: out pointer to a local.
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    pid == std::process::id()
}

fn class_name(h: HWND) -> String {
    let mut buf = [0u16; 128];
    // SAFETY: buffer of the given length.
    let n = unsafe { GetClassNameW(h, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// The program behind a window for the log; never its title, which can show private things.
fn describe(h: HWND) -> String {
    let program = win32::window_program(Hwnd::from_raw(h));
    if program.is_empty() { class_name(h) } else { program }
}
