//! Win32 helpers for the overlay window (PLAN.md 6.1–6.3).
//!
//! Why direct Win32 instead of Tauri setters: tao recomputes the full extended window
//! style on every window-flag change (`WindowFlags::apply_diff`) and drops bits it does
//! not know, such as `WS_EX_TOOLWINDOW`. It also calls `ShowWindow(SW_SHOW)` after every
//! flag change on a visible window, which can activate the overlay and steal focus from
//! the game. Spike B compares both approaches (see `selftest.rs`).

use windows::Win32::Foundation::{COLORREF, CloseHandle, HWND, LPARAM, POINT, RECT};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, EnumDisplayMonitors, GetDC, GetDIBits, GetMonitorInfoW, HDC, HMONITOR,
    MONITORINFO, MONITORINFOEXW, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GA_ROOT, GW_HWNDNEXT, GWL_EXSTYLE, GetAncestor, GetForegroundWindow, GetLayeredWindowAttributes,
    GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, HWND_TOPMOST, IsWindowVisible,
    LAYERED_WINDOW_ATTRIBUTES_FLAGS, LWA_ALPHA, MONITORINFOF_PRIMARY, SW_HIDE, SW_SHOWNOACTIVATE, SWP_ASYNCWINDOWPOS,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WS_EX_APPWINDOW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WindowFromPoint,
};
use windows::core::{BOOL, HSTRING, PCWSTR, PWSTR};

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
pub const OVERLAY_REQUIRED: WINDOW_EX_STYLE =
    WINDOW_EX_STYLE(WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0);
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
    if !layered_alpha_set(h) {
        set_layered_alpha(h, 255);
    }
    reassert_topmost(h, changed);
    changed
}

/// True if all bits the overlay needs for this click-through state are present.
pub fn exstyles_ok(h: Hwnd, click_through: bool) -> bool {
    let cur = ex_style(h);
    wanted_ex_style(cur, click_through) == cur && layered_alpha_set(h)
}

/// Whether the layered window has constant-alpha attributes. Without them Windows hit-tests
/// the layered window as fully transparent, because WebView2 draws through DirectComposition
/// and never into the surface Windows checks: clicks fall through even with
/// WS_EX_TRANSPARENT off, so the edit mode could not be dragged (found in Phase 1). The
/// attributes do not change what is drawn.
pub fn layered_alpha_set(h: Hwnd) -> bool {
    let mut flags = LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
    // SAFETY: valid out pointer; fails for a layered window that never had attributes set.
    unsafe { GetLayeredWindowAttributes(h.raw(), None, None, Some(&mut flags)).is_ok() && flags.0 & LWA_ALPHA.0 != 0 }
}

/// Moves the window to the top of the topmost band without activating it.
pub fn reassert_topmost(h: Hwnd, frame_changed: bool) {
    let mut flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
    if frame_changed {
        flags |= SWP_FRAMECHANGED;
    }
    // SAFETY: z-order change of our own window.
    unsafe {
        let _ = SetWindowPos(h.raw(), Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
    }
}

/// Sets constant alpha on a layered window. Some layered windows stay invisible until
/// SetLayeredWindowAttributes or UpdateLayeredWindow has been called once, and hit-testing
/// needs it (see `layered_alpha_set`).
pub fn set_layered_alpha(h: Hwnd, alpha: u8) -> bool {
    // SAFETY: attribute change on our own layered window.
    unsafe { SetLayeredWindowAttributes(h.raw(), COLORREF(0), alpha, LWA_ALPHA).is_ok() }
}

/// Moves and resizes in one step, without activating and without touching the z-order.
/// Replaces Tauri's `set_size`/`set_position`, which tao applies as two separate moves.
/// Asynchronous when called off the window's thread: the caller may hold a lock that the
/// main thread wants, and a synchronous cross-thread SetWindowPos would wait for it.
pub fn set_bounds(h: Hwnd, x: i32, y: i32, width: i32, height: i32) {
    let flags = SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS;
    // SAFETY: geometry change of our own window.
    unsafe {
        let _ = SetWindowPos(h.raw(), None, x, y, width, height, flags);
    }
}

/// One display as Windows reports it, in physical pixels of the virtual screen.
#[derive(Clone, Debug, PartialEq)]
pub struct RawMonitor {
    /// GDI device name such as `\\.\DISPLAY1`; stable across restarts for the same port.
    pub device: String,
    pub rect: RECT,
    pub primary: bool,
    /// Effective DPI; 96 = 100 %.
    pub dpi: u32,
}

/// All displays in enumeration order. Thread-agnostic, unlike Tauri's monitor getters.
pub fn monitors() -> Vec<RawMonitor> {
    unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: `data` is the Vec passed below and outlives the enumeration.
        let list = unsafe { &mut *(data.0 as *mut Vec<RawMonitor>) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: MONITORINFOEXW starts with MONITORINFO; cbSize tells Windows the real size.
        if unsafe { GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) }.as_bool() {
            let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
            // SAFETY: valid out pointers; on failure the 96 DPI default stays.
            let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
            let len = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
            list.push(RawMonitor {
                device: String::from_utf16_lossy(&info.szDevice[..len]),
                rect: info.monitorInfo.rcMonitor,
                primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                dpi: dpi_x,
            });
        }
        true.into()
    }
    let mut list: Vec<RawMonitor> = Vec::new();
    // SAFETY: the callback only touches `list` during this call.
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut list as *mut Vec<RawMonitor> as isize));
    }
    list
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

/// The program that owns a window, as its file name (for example "Discord.exe"). Logs, the
/// diagnostics page and self-test reports use this instead of window titles, which can show
/// private things such as chat servers, documents or the song playing.
pub fn window_program(h: Hwnd) -> String {
    if h.is_null() {
        return String::new();
    }
    let mut pid = 0u32;
    // SAFETY: out pointer to a local.
    unsafe { GetWindowThreadProcessId(h.raw(), Some(&mut pid)) };
    if pid == 0 {
        return String::new();
    }
    // SAFETY: plain query; the handle is closed below.
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return format!("pid {pid}");
    };
    let mut buf = [0u16; 512];
    let mut len = buf.len() as u32;
    // SAFETY: buffer and length describe the same local array; the handle is open.
    let ok = unsafe { QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len) }.is_ok();
    // SAFETY: handle from OpenProcess, closed once.
    let _ = unsafe { CloseHandle(process) };
    if !ok {
        return format!("pid {pid}");
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    path.rsplit(['\\', '/']).next().unwrap_or_default().to_string()
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
    unsafe { FindWindowW(PCWSTR::null(), &HSTRING::from(title)).ok().filter(|h| !h.is_invalid()).map(Hwnd::from_raw) }
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

/// Hands the foreground back to a window. Only works while our process may set it,
/// e.g. right after one of our windows took it (used by the self-test to repair a steal).
pub fn set_foreground(h: Hwnd) -> bool {
    // SAFETY: plain call with a window handle.
    unsafe { SetForegroundWindow(h.raw()).as_bool() }
}

/// Captures the composed screen (including layered windows) inside `r` as 0x00RRGGBB pixels.
pub fn capture_rect(r: &RECT) -> Option<Vec<u32>> {
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    if w <= 0 || h <= 0 {
        return None;
    }
    // SAFETY: standard GDI capture into a memory bitmap; every handle is released.
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return None;
        }
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp.into());
        let blit_ok = BitBlt(mem, 0, 0, w, h, Some(screen), r.left, r.top, SRCCOPY | CAPTUREBLT).is_ok();
        SelectObject(mem, old);

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // negative = top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            ..Default::default()
        };
        let mut pixels = vec![0u32; (w * h) as usize];
        let lines = GetDIBits(mem, bmp, 0, h as u32, Some(pixels.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        (blit_ok && lines == h).then_some(pixels)
    }
}

/// Share of pixels whose strongest colour channel changed by more than `threshold`.
pub fn changed_share(before: &[u32], after: &[u32], threshold: i32) -> f64 {
    if before.is_empty() || before.len() != after.len() {
        return 0.0;
    }
    let channel = |p: u32, shift: u32| ((p >> shift) & 0xFF) as i32;
    let changed = before
        .iter()
        .zip(after)
        .filter(|&(&a, &b)| [0, 8, 16].iter().any(|&s| (channel(a, s) - channel(b, s)).abs() > threshold))
        .count();
    changed as f64 / before.len() as f64
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs a desktop session (the taskbar), so it only runs on request: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn window_program_names_the_taskbar_owner() {
        // SAFETY: plain query by class name.
        let bar = unsafe { FindWindowW(&HSTRING::from("Shell_TrayWnd"), PCWSTR::null()) }.expect("taskbar window");
        assert_eq!(window_program(Hwnd::from_raw(bar)).to_lowercase(), "explorer.exe");
    }
}
