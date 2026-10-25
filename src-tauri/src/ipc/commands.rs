//! Commands, frontend → backend (PLAN.md 4.4). Commands that change the overlay are `async`
//! so they run on the async runtime and never block the main thread.

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

use crate::app_state::{self, LatencySample, NowPlaying, NowPlayingState};
use crate::overlay::{self, EditDraft, LayoutEvent, OverlayDiagnostics, window::Monitor};
use crate::settings::{Anchor, OverlaySettings, Settings, SettingsState};

#[tauri::command]
pub fn get_settings(settings: State<'_, SettingsState>) -> Settings {
    settings.get()
}

/// Applies a JSON Merge Patch such as `{ "overlay": { "scale": 1.2 } }`.
#[tauri::command]
pub async fn set_settings(app: AppHandle, patch: Value) -> Result<Settings, String> {
    let (before, after) = app.state::<SettingsState>().patch(&patch)?;
    super::settings_changed(&app, &before, &after);
    Ok(after)
}

/// Picks an anchor and moves the overlay to its snap point.
#[tauri::command]
pub async fn set_overlay_anchor(app: AppHandle, anchor: Anchor) -> Settings {
    let monitors = overlay::window::list_monitors();
    let (before, after) = app.state::<SettingsState>().update(|s| {
        let m = overlay::window::pick(&monitors, s.overlay.monitor_id.as_deref());
        let (x, y) = m.map(|m| overlay::window::snapped_offsets(anchor, m)).unwrap_or((0.0, 0.0));
        s.overlay.anchor = anchor;
        s.overlay.offset_x_pct = x;
        s.overlay.offset_y_pct = y;
    });
    super::settings_changed(&app, &before, &after);
    after
}

/// Restores position and size defaults; keeps the pause setting.
#[tauri::command]
pub async fn reset_overlay_geometry(app: AppHandle) -> Settings {
    let (before, after) = app.state::<SettingsState>().update(|s| {
        let keep = s.overlay.hide_after_pause_s;
        s.overlay = OverlaySettings { hide_after_pause_s: keep, ..OverlaySettings::default() };
    });
    super::settings_changed(&app, &before, &after);
    after
}

#[tauri::command]
pub fn list_monitors() -> Vec<Monitor> {
    overlay::window::list_monitors()
}

#[tauri::command]
pub fn get_now_playing(state: State<'_, NowPlayingState>) -> NowPlaying {
    state.snapshot()
}

#[tauri::command]
pub fn track_latencies(state: State<'_, NowPlayingState>) -> Vec<LatencySample> {
    state.latencies()
}

#[tauri::command]
pub async fn get_overlay_layout(app: AppHandle) -> Option<LayoutEvent> {
    overlay::current_layout(&app)
}

#[tauri::command]
pub async fn report_overlay_size(app: AppHandle, width: f64, height: f64, ui_scale: f64) {
    overlay::report_content_size(&app, width, height, ui_scale);
}

#[tauri::command]
pub async fn overlay_layout_ready(app: AppHandle, seq: u64) {
    overlay::layout_ready(&app, seq);
}

#[tauri::command]
pub async fn overlay_rendered(app: AppHandle, key: String) {
    app_state::rendered(&app, &key);
}

#[tauri::command]
pub async fn enter_edit_mode(app: AppHandle) {
    overlay::enter_edit(&app);
}

#[tauri::command]
pub async fn overlay_edit_update(app: AppHandle, draft: EditDraft) {
    overlay::edit_update(&app, draft);
}

#[tauri::command]
pub async fn overlay_edit_monitor(app: AppHandle, step: i32) {
    overlay::edit_switch_monitor(&app, step);
}

#[tauri::command]
pub async fn save_overlay_geometry(app: AppHandle, draft: Option<EditDraft>) {
    overlay::save_edit(&app, draft);
}

#[tauri::command]
pub async fn cancel_edit_mode(app: AppHandle) {
    overlay::cancel_edit(&app);
}

#[tauri::command]
pub async fn toggle_overlay(app: AppHandle) -> bool {
    overlay::toggle(&app)
}

#[tauri::command]
pub async fn overlay_diagnostics(app: AppHandle) -> OverlayDiagnostics {
    overlay::diagnostics(&app)
}

#[tauri::command]
pub async fn run_overlay_selftest(app: AppHandle) -> Result<overlay::selftest::Report, String> {
    tauri::async_runtime::spawn_blocking(move || overlay::selftest::run_and_record(&app))
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct AppInfo {
    pub version: &'static str,
    pub settings_path: String,
    pub log_dir: String,
    /// Whether Windows actually has the autostart entry.
    pub autostart_registered: Option<bool>,
}

#[tauri::command]
pub fn app_info(app: AppHandle, settings: State<'_, SettingsState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        settings_path: settings.path().display().to_string(),
        log_dir: crate::logging::log_dir().display().to_string(),
        autostart_registered: app.autolaunch().is_enabled().ok(),
    }
}

#[tauri::command]
pub fn open_logs_folder(app: AppHandle) -> Result<(), String> {
    let dir = crate::logging::log_dir();
    app.opener().open_path(dir.display().to_string(), None::<&str>).map_err(|e| e.to_string())
}
