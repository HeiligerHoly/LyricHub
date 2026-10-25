//! LyricHub backend. Phase 0: tray, two windows, overlay spike, logging, SMTC snapshot.

mod logging;
mod overlay;
mod smtc;
mod tray;

use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[tauri::command]
fn overlay_diagnostics(app: AppHandle) -> overlay::OverlayDiagnostics {
    overlay::diagnostics(&app)
}

#[tauri::command]
fn toggle_overlay(app: AppHandle) -> bool {
    overlay::toggle(&app)
}

#[tauri::command]
fn set_click_through(app: AppHandle, enabled: bool) -> overlay::OverlayDiagnostics {
    overlay::set_click_through(&app, enabled);
    overlay::diagnostics(&app)
}

#[tauri::command]
async fn now_playing_snapshot() -> Result<Option<smtc::NowPlaying>, String> {
    tauri::async_runtime::spawn_blocking(|| smtc::snapshot("Spotify"))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn run_overlay_selftest(app: AppHandle) -> Result<overlay::selftest::Report, String> {
    tauri::async_runtime::spawn_blocking(move || overlay::selftest::run_and_record(&app))
        .await
        .map_err(|e| e.to_string())
}

/// Shows and focuses the settings window. The overlay never gets focus; this window may.
pub fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn register_shortcuts(app: &AppHandle) -> tauri::Result<()> {
    let toggle = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyL);
    let edit = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyE);
    let (toggle_h, edit_h) = (toggle.clone(), edit.clone());
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if event.state() != ShortcutState::Pressed {
                    return;
                }
                if shortcut == &toggle_h {
                    overlay::toggle(app);
                } else if shortcut == &edit_h {
                    // Phase 0: only toggles click-through. The real edit mode follows in Phase 1.
                    overlay::toggle_click_through(app);
                }
            })
            .build(),
    )?;
    for (name, s) in [("Ctrl+Alt+L", toggle), ("Ctrl+Alt+E", edit)] {
        if let Err(e) = app.global_shortcut().register(s) {
            // Another app may own the combination; the tray still works.
            tracing::warn!(shortcut = name, error = %e, "could not register global shortcut");
        }
    }
    Ok(())
}

pub fn run() {
    logging::init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "LyricHub starting");

    let minimized = std::env::args().any(|a| a == "--minimized");
    let selftest_mode = std::env::var("LYRICHUB_SELFTEST").ok();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| open_settings(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .manage(overlay::OverlayState::default())
        .invoke_handler(tauri::generate_handler![
            overlay_diagnostics,
            toggle_overlay,
            set_click_through,
            now_playing_snapshot,
            run_overlay_selftest
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            register_shortcuts(&handle)?;
            tray::create(&handle)?;
            overlay::setup(&handle)?;

            if let Some(mode) = selftest_mode.clone() {
                // Give the webview time to load and paint before measuring.
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(3000));
                    let report = overlay::selftest::run_and_record(&handle);
                    if mode == "exit" {
                        handle.exit(if report.failed == 0 { 0 } else { 1 });
                    }
                });
            } else if !minimized {
                open_settings(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the settings window keeps the app running in the tray.
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running LyricHub");
}
