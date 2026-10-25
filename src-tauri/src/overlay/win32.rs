//! Win32 helpers for the overlay window (PLAN.md 6.1–6.3).
//!
//! Why direct Win32 instead of Tauri setters: tao recomputes the full extended window
//! style on every window-flag change (`WindowFlags::apply_diff`) and drops bits it does
//! not know, such as `WS_EX_TOOLWINDOW`. It also calls `ShowWindow(SW_SHOW)` after every
//! flag change on a visible window, which can activate the overlay and steal focus from
//! the game. Spike B compares both approaches (see `selftest.rs`).

use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CAPTUREBLT, CLR_INVALID, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
    GetPixel, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GA_ROOT, GW_HWNDNEXT, GWL_EXSTYLE, GetAncestor, GetForegroundWindow, GetWindow, GetWindowLongPtrW,
    GetWindowRect, GetWindowTextW, HWND_TOPMOST, IsWindowVisible, LWA_ALPHA, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetLayeredWindowAttributes, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WindowFromPoint,
};
use windows::core::{HSTRING, PCWSTR};

/// `HWND` wraps a raw pointer and is not `Send`. This copyable handle can cross threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Hwnd(pub isize);

impl Hwnd {
    pub fn from_raw(h: HWND) -> Self {
        Self(h.0 as isize)
    }
    pub fn raw(self) -> HWND {
        HWND(self.0 as *mut core::ffi::c_void)
    }
    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

/// Extended styles the overlay must always carry, independent of click-through.
pub const OVERLAY_REQUIRED: WINDOW_EX_STYLE = WINDOW_EX_STYLE(WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0);
/// Extra bits for click-through: layered + transparent makes hit-testing skip the window.
pub const CLICK_THROUGH: WINDOW_EX_STYLE = WINDOW_EX_STYLE(WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0);

pub fn ex_style(h: Hwnd) -> WINDOW_EX_STYLE {
    // SAFETY: plain query on a window handle; an invalid handle returns 0.
    unsafe { WINDOW_EX_STYLE(GetWindowLongPtrW(h.raw(), GWL_EXSTYLE) as u32) }
}

fn set_ex_style(h: Hwnd, style: WINDOW_EX_STYLE) {
    // SAFETY: changes style bits of our own window.
    unsafe {
        SetWindowLongPtrW(h.raw(), GWL_EXSTYLE, style.0 as isize);
    }
}

/// The style the overlay should have for the given click-through state.
pub fn wanted_ex_style(current: WINDOW_EX_STYLE, click_through: bool) -> WINDOW_EX_STYLE {
    let mut want = current.0 | OVERLAY_REQUIRED.0;
    want &= !WS_EX_APPWINDOW.0;
    if click_through {
        want |= CLICK_THROUGH.0;
    } else {
        // WS_EX_LAYERED stays: removing it recreates the redirection surface and flickers.
        want &= !WS_EX_TRANSPARENT.0;
    }
    WINDOW_EX_STYLE(want)
}

/// Applies the overlay's extended styles and re-asserts topmost without activating.
/// Returns true if the style had to be changed.
pub fn apply_overlay_exstyles(h: Hwnd, click_through: bool) -> bool {
    let before = ex_style(h);
    let want = wanted_ex_style(before, click_through);
    let changed = want != before;
    if changed {
        set_ex_style(h, want);
    }
    reassert_topmost(h, changed);
    changed
}

/// True if all bits the overlay needs for this click-through state are present.
pub fn exstyles_ok(h: Hwnd, click_through: bool) -> bool {
    let cur = ex_style(h);
    wanted_ex_style(cur, click_through) == cur
}

/// Moves the window to the top of the topmost band without activating it.
pub fn reassert_topmost(h: Hwnd, frame_changed: bool) {
    let mut flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
    if frame_changed {
        flags = flags | SWP_FRAMECHANGED;
    }
    // SAFETY: z-order change of our own window.
    unsafe {
        let _ = SetWindowPos(h.raw(), Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
    }
}

/// Sets constant alpha on a layered window. Some layered windows stay invisible until
/// SetLayeredWindowAttributes or UpdateLayeredWindow has been called once.
pub fn set_layered_alpha(h: Hwnd, alpha: u8) -> bool {
    // SAFETY: attribute change on our own layered window.
    unsafe { SetLayeredWindowAttributes(h.raw(), COLORREF(0), alpha, LWA_ALPHA).is_ok() }
}

pub fn show_no_activate(h: Hwnd) {
    // SAFETY: shows our own window without activating it.
    unsafe {
        let _ = ShowWindow(h.raw(), SW_SHOWNOACTIVATE);
    }
}

pub fn hide(h: Hwnd) {
    // SAFETY: hides our own window.
    unsafe {
        let _ = ShowWindow(h.raw(), SW_HIDE);
    }
}

pub fn is_visible(h: Hwnd) -> bool {
    // SAFETY: plain query.
    unsafe { IsWindowVisible(h.raw()).as_bool() }
}

pub fn foreground() -> Hwnd {
    // SAFETY: plain query.
    Hwnd::from_raw(unsafe { GetForegroundWindow() })
}

pub fn window_title(h: Hwnd) -> String {
    if h.is_null() {
        return String::new();
    }
    let mut buf = [0u16; 256];
    // SAFETY: buffer is valid for its full length.
    let len = unsafe { GetWindowTextW(h.raw(), &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

pub fn window_rect(h: Hwnd) -> Option<RECT> {
    let mut r = RECT::default();
    // SAFETY: r is a valid out pointer.
    unsafe { GetWindowRect(h.raw(), &mut r).ok().map(|_| r) }
}

/// Top-level window that would receive a mouse click at this screen point.
pub fn root_window_from_point(x: i32, y: i32) -> Hwnd {
    // SAFETY: plain queries.
    unsafe {
        let hit = WindowFromPoint(POINT { x, y });
        if hit.is_invalid() {
            return Hwnd::default();
        }
        Hwnd::from_raw(GetAncestor(hit, GA_ROOT))
    }
}

pub fn find_window_by_title(title: &str) -> Option<Hwnd> {
    // SAFETY: plain query with a valid wide string.
    unsafe {
        FindWindowW(PCWSTR::null(), &HSTRING::from(title))
            .ok()
            .filter(|h| !h.is_invalid())
            .map(Hwnd::from_raw)
    }
}

/// True if `upper` is above `lower` in the z-order, None if `lower` was not found below.
pub fn is_above(upper: Hwnd, lower: Hwnd) -> Option<bool> {
    let mut cur = upper;
    for _ in 0..4096 {
        // SAFETY: walks the z-order; stops at the end of the list.
        let next = unsafe { GetWindow(cur.raw(), GW_HWNDNEXT) };
        match next {
            Ok(n) if !n.is_invalid() => {
                let n = Hwnd::from_raw(n);
                if n == lower {
                    return Some(true);
                }
                cur = n;
            }
            _ => break,
        }
    }
    // Not below us: check the other direction to distinguish "below" from "unknown".
    let mut cur = lower;
    for _ in 0..4096 {
        // SAFETY: as above.
        let next = unsafe { GetWindow(cur.raw(), GW_HWNDNEXT) };
        match next {
            Ok(n) if !n.is_invalid() => {
                let n = Hwnd::from_raw(n);
                if n == upper {
                    return Some(false);
                }
                cur = n;
            }
            _ => break,
        }
    }
    None
}

/// Reads one composed screen pixel (including layered windows) as 0x00BBGGRR.
pub fn screen_pixel(x: i32, y: i32) -> Option<u32> {
    // SAFETY: standard GDI capture into a 1x1 memory bitmap; every handle is released.
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return None;
        }
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, 1, 1);
        let old = SelectObject(mem, bmp.into());
        let ok = BitBlt(mem, 0, 0, 1, 1, Some(screen), x, y, SRCCOPY | CAPTUREBLT).is_ok();
        let px = GetPixel(mem, 0, 0);
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        (ok && px.0 != CLR_INVALID).then_some(px.0)
    }
}

/// Human-readable list of the style bits Spike B cares about.
pub fn describe_ex_style(s: WINDOW_EX_STYLE) -> String {
    let names = [
        (WS_EX_TOPMOST, "TOPMOST"),
        (WS_EX_NOACTIVATE, "NOACTIVATE"),
        (WS_EX_TOOLWINDOW, "TOOLWINDOW"),
        (WS_EX_LAYERED, "LAYERED"),
        (WS_EX_TRANSPARENT, "TRANSPARENT"),
        (WS_EX_APPWINDOW, "APPWINDOW"),
    ];
    let set: Vec<&str> = names.iter().filter(|(b, _)| s.0 & b.0 != 0).map(|(_, n)| *n).collect();
    format!("0x{:08X} [{}]", s.0, set.join(" "))
}

pub fn has(s: WINDOW_EX_STYLE, bit: WINDOW_EX_STYLE) -> bool {
    s.0 & bit.0 != 0
}

pub use windows::Win32::UI::WindowsAndMessaging::{
    WS_EX_APPWINDOW as EX_APPWINDOW, WS_EX_LAYERED as EX_LAYERED, WS_EX_NOACTIVATE as EX_NOACTIVATE,
    WS_EX_TOOLWINDOW as EX_TOOLWINDOW, WS_EX_TOPMOST as EX_TOPMOST, WS_EX_TRANSPARENT as EX_TRANSPARENT,
};
