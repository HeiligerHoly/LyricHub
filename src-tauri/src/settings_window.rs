//! The settings window exists only while it is open (PLAN.md 10, Phase 1): it is created on
//! demand and destroyed on close, which saves about 30 MB of WebView2 memory (Spike B).

use tauri::webview::Color;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "main";

/// Shows and focuses the settings window, creating it if needed. The overlay never gets
/// focus; this window may, because the user asked for it.
pub fn open(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let app = app.clone();
    // Building a webview window waits for the main thread, so it must not run on it.
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("/".into()))
            .title("LyricHub")
            .inner_size(960.0, 640.0)
            .min_inner_size(720.0, 480.0)
            .center()
            // Same colour as the page, so opening does not flash white.
            .background_color(Color(17, 19, 24, 255))
            .build();
        match built {
            Ok(w) => {
                let _ = w.set_focus();
                tracing::info!("settings window opened");
            }
            // A second click while the first window is still being built lands here.
            Err(e) => tracing::debug!(error = %e, "settings window not created"),
        }
    });
}
