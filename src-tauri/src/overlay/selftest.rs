//! Spike B self-test (PLAN.md 10, Phase 0): checks the overlay window without a human.
//!
//! Each step records what Windows reports: foreground window, extended styles,
//! WindowFromPoint (click-through), a composed screen pixel (is anything drawn) and
//! z-order against an optional fake game window. Two strategies are compared:
//!   - "win32": state changes via ShowWindow/SetWindowLongPtr (the production path)
//!   - "tauri": state changes via Tauri's show/hide/set_ignore_cursor_events
//!
//! Start it via tray, the settings page, or `LYRICHUB_SELFTEST=1` (`=exit` quits afterwards).
//! Results go to the log, to stdout as one `SELFTEST_RESULT {json}` line, and to
//! `%APPDATA%/LyricHub/logs/selftest-<unix>.json`.

use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::win32::{self, Hwnd};
use super::{OverlayState, window};

/// Title of the window created by `tools/overlay-spike/fake-game.ps1`.
pub const FAKE_GAME_TITLE: &str = "LyricHub Fake Game";
const SETTLE: Duration = Duration::from_millis(400);

#[derive(Serialize, Clone, Debug)]
pub struct Step {
    pub group: String,
    pub name: String,
    /// "pass", "fail" or "info" (not conclusive, or a measurement without threshold)
    pub status: &'static str,
    pub detail: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Report {
    pub unix_time: u64,
    pub foreground_at_start: String,
    pub fake_game_present: bool,
    pub passed: usize,
    pub failed: usize,
    pub steps: Vec<Step>,
}

struct Ctx {
    steps: Vec<Step>,
    group: &'static str,
    fg0: Hwnd,
    fg_conclusive: bool,
    overlay: Hwnd,
}

impl Ctx {
    fn push(&mut self, name: &str, status: &'static str, detail: impl Into<String>) {
        let detail = detail.into();
        tracing::info!(group = self.group, step = name, status, %detail, "selftest");
        self.steps.push(Step { group: self.group.into(), name: name.into(), status, detail });
    }
    fn check(&mut self, name: &str, ok: bool, detail: impl Into<String>) {
        self.push(name, if ok { "pass" } else { "fail" }, detail);
    }
    /// The overlay must never take the foreground from whatever window had it.
    fn focus_kept(&mut self, name: &str) {
        let fg = win32::foreground();
        let detail = format!("Vordergrund: \"{}\"", win32::window_title(fg));
        if !self.fg_conclusive {
            self.push(name, "info", format!("{detail} (Startzustand ohne fremdes Vordergrundfenster, nicht aussagekräftig)"));
        } else if fg == self.overlay {
            self.push(name, "fail", format!("Overlay hat den Fokus übernommen. {detail}"));
        } else {
            self.check(name, fg == self.fg0, detail);
        }
    }
    fn styles(&mut self, name: &str, click_through: bool) {
        let s = win32::ex_style(self.overlay);
        self.check(name, win32::exstyles_ok(self.overlay, click_through), win32::describe_ex_style(s));
    }
    fn click_through(&mut self, name: &str, expect_through: bool) {
        let Some(r) = win32::window_rect(self.overlay) else {
            self.push(name, "fail", "Fensterrechteck nicht lesbar");
            return;
        };
        let (cx, cy) = ((r.left + r.right) / 2, (r.top + r.bottom) / 2);
        let hit = win32::root_window_from_point(cx, cy);
        let through = hit != self.overlay;
        let detail = format!("Treffer bei ({cx},{cy}): \"{}\"", win32::window_title(hit));
        self.check(name, through == expect_through, detail);
    }
}

fn probe_point(h: Hwnd) -> Option<(i32, i32)> {
    // Inside the glass panel: 60 px from the left edge, vertically centred.
    win32::window_rect(h).map(|r| (r.left + 60, (r.top + r.bottom) / 2))
}

pub fn run(app: &AppHandle) -> Report {
    let state = app.state::<OverlayState>();
    let overlay = state.hwnd();
    state.pause_watchdog(true);

    let fg0 = win32::foreground();
    let main = app.get_webview_window("main").and_then(|w| w.hwnd().ok()).map(Hwnd::from_raw);
    let fg_conclusive = !fg0.is_null() && fg0 != overlay && Some(fg0) != main;
    let fake_game = win32::find_window_by_title(FAKE_GAME_TITLE);
    let mut c = Ctx { steps: Vec::new(), group: "start", fg0, fg_conclusive, overlay };
    c.push(
        "Ausgangslage",
        "info",
        format!(
            "Vordergrund: \"{}\", Fake-Game: {}",
            win32::window_title(fg0),
            if fake_game.is_some() { "ja" } else { "nein" }
        ),
    );

    // Baseline pixel with the overlay hidden.
    win32::hide(overlay);
    sleep(SETTLE);
    let probe = probe_point(overlay);
    let px_hidden = probe.and_then(|(x, y)| win32::screen_pixel(x, y));

    // ---- Strategy A: direct Win32 (production path) ----
    c.group = "win32";
    win32::apply_overlay_exstyles(overlay, true);
    win32::show_no_activate(overlay);
    sleep(SETTLE);
    c.focus_kept("Einblenden ohne Fokusverlust");
    c.styles("Stil-Bits vollständig (klickdurchlässig)", true);

    let px_shown = probe.and_then(|(x, y)| win32::screen_pixel(x, y));
    let drawn = px_hidden.is_some() && px_shown.is_some() && px_hidden != px_shown;
    if drawn {
        c.push("Overlay wird gezeichnet", "pass", format!("Pixel vorher {px_hidden:06X?}, nachher {px_shown:06X?}"));
    } else {
        // Layered windows can stay invisible until SetLayeredWindowAttributes was called once.
        let attr_ok = win32::set_layered_alpha(overlay, 255);
        sleep(SETTLE);
        let px_after = probe.and_then(|(x, y)| win32::screen_pixel(x, y));
        let drawn_after = px_hidden.is_some() && px_after.is_some() && px_hidden != px_after;
        c.push(
            "Overlay wird gezeichnet",
            if drawn_after { "info" } else { "fail" },
            format!(
                "Pixel vorher {px_hidden:06X?}, nachher {px_shown:06X?}; nach SetLayeredWindowAttributes(ok={attr_ok}) {px_after:06X?}. \
                 Bei gesperrtem Bildschirm oder Remote-Sitzung ist dieser Test nicht aussagekräftig."
            ),
        );
    }

    c.click_through("Klicks gehen durch", true);
    c.push("Alt+Tab", if win32::has(win32::ex_style(overlay), win32::EX_TOOLWINDOW) { "pass" } else { "fail" },
        "TOOLWINDOW gesetzt und APPWINDOW nicht gesetzt hält das Fenster aus Alt+Tab heraus (Sichtprüfung empfohlen)");

    win32::apply_overlay_exstyles(overlay, false);
    sleep(SETTLE);
    c.focus_kept("Klickdurchlässigkeit aus, ohne Fokusverlust");
    c.styles("Stil-Bits vollständig (Edit-Modus)", false);
    c.click_through("Edit-Modus fängt Klicks ab", false);

    win32::apply_overlay_exstyles(overlay, true);
    sleep(SETTLE);
    c.focus_kept("Klickdurchlässigkeit wieder an, ohne Fokusverlust");
    c.click_through("Klicks gehen wieder durch", true);

    win32::hide(overlay);
    sleep(SETTLE);
    c.check("Ausblenden", !win32::is_visible(overlay), "IsWindowVisible nach SW_HIDE");
    c.focus_kept("Ausblenden ohne Fokusverlust");
    win32::show_no_activate(overlay);
    sleep(SETTLE);
    c.focus_kept("Erneutes Einblenden ohne Fokusverlust");

    // ---- Strategy B: Tauri setters (documents tao behaviour) ----
    c.group = "tauri";
    if let Some(win) = window(app) {
        let _ = win.set_ignore_cursor_events(false);
        sleep(SETTLE);
        c.focus_kept("set_ignore_cursor_events(false) ohne Fokusverlust");
        tool_window_kept(&mut c, "TOOLWINDOW bleibt nach set_ignore_cursor_events(false)");

        let _ = win.set_ignore_cursor_events(true);
        sleep(SETTLE);
        c.focus_kept("set_ignore_cursor_events(true) ohne Fokusverlust");
        tool_window_kept(&mut c, "TOOLWINDOW bleibt nach set_ignore_cursor_events(true)");
        c.click_through("Klicks gehen durch (Tauri)", true);

        let _ = win.hide();
        sleep(SETTLE);
        let _ = win.show();
        sleep(SETTLE);
        c.focus_kept("Tauri hide()/show() ohne Fokusverlust");
        tool_window_kept(&mut c, "TOOLWINDOW bleibt nach Tauri show()");
    } else {
        c.push("Tauri-Strategie", "fail", "Overlay-Fenster nicht gefunden");
    }

    // Restore the production state.
    win32::apply_overlay_exstyles(overlay, true);
    win32::show_no_activate(overlay);
    state.click_through.store(true, std::sync::atomic::Ordering::SeqCst);
    state.visible.store(true, std::sync::atomic::Ordering::SeqCst);

    // ---- Topmost against a fake game that holds the topmost band itself ----
    c.group = "topmost";
    if let Some(game) = fake_game {
        let (mut above, mut total) = (0u32, 0u32);
        for i in 0..60 {
            if i % 20 == 0 {
                win32::reassert_topmost(overlay, false); // what the watchdog does every 2 s
            }
            sleep(Duration::from_millis(100));
            if let Some(a) = win32::is_above(overlay, game) {
                total += 1;
                above += a as u32;
            }
        }
        let share = if total > 0 { 100.0 * above as f64 / total as f64 } else { 0.0 };
        c.push(
            "Overlay über Fake-Game (6 s, Reassert alle 2 s)",
            if share >= 95.0 { "pass" } else if total == 0 { "fail" } else { "info" },
            format!("{share:.0} % der Proben oben ({above}/{total})"),
        );
        c.click_through("Klicks landen im Fake-Game", true);
    } else {
        c.push("Fake-Game", "info", "nicht gestartet; tools/overlay-spike/fake-game.ps1 für diesen Test starten");
    }

    state.pause_watchdog(false);

    let passed = c.steps.iter().filter(|s| s.status == "pass").count();
    let failed = c.steps.iter().filter(|s| s.status == "fail").count();
    Report {
        unix_time: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        foreground_at_start: win32::window_title(fg0),
        fake_game_present: fake_game.is_some(),
        passed,
        failed,
        steps: c.steps,
    }
}

fn tool_window_kept(c: &mut Ctx, name: &str) {
    let s = win32::ex_style(c.overlay);
    let kept = win32::has(s, win32::EX_TOOLWINDOW) && !win32::has(s, win32::EX_APPWINDOW);
    c.check(name, kept, win32::describe_ex_style(s));
}

/// Runs the self-test and writes the report to stdout and the log directory.
pub fn run_and_record(app: &AppHandle) -> Report {
    let report = run(app);
    match serde_json::to_string(&report) {
        Ok(json) => {
            println!("SELFTEST_RESULT {json}");
            let path = crate::logging::log_dir().join(format!("selftest-{}.json", report.unix_time));
            if let Err(e) = std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap_or(json)) {
                tracing::warn!(error = %e, "could not write selftest report");
            } else {
                tracing::info!(path = %path.display(), passed = report.passed, failed = report.failed, "selftest report written");
            }
        }
        Err(e) => tracing::error!(error = %e, "could not serialise selftest report"),
    }
    report
}
