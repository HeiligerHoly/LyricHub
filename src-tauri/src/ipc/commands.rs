//! Commands, frontend → backend (PLAN.md 4.4). Commands that change the overlay are `async`
//! so they run on the async runtime and never block the main thread.

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

use crate::app_state::{self, ClockEvent, LatencySample, NowPlaying, NowPlayingState};
use crate::lyrics::types::LyricsEvent;
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
pub fn get_lyrics(state: State<'_, NowPlayingState>) -> Option<LyricsEvent> {
    state.lyrics()
}

#[tauri::command]
pub async fn get_clock(app: AppHandle) -> ClockEvent {
    app_state::clock_event(&app, "query")
}

/// Shifts the offset of the current song by `delta_ms`; positive shows the lyrics later.
#[tauri::command]
pub async fn nudge_offset(app: AppHandle, delta_ms: i32) -> Option<i32> {
    super::nudge(&app, delta_ms)
}

#[tauri::command]
pub async fn sync_lab(app: AppHandle) -> crate::sync_lab::SyncLab {
    crate::sync_lab::snapshot(&app)
}

/// Writes the diagnostics export and shows it in Explorer; returns its path.
#[tauri::command]
pub async fn export_diagnostics(app: AppHandle) -> Result<String, String> {
    let path = crate::sync_lab::export(&app)?;
    let _ = app.opener().reveal_item_in_dir(&path);
    Ok(path.display().to_string())
}

#[tauri::command]
pub async fn start_calibration(
    app: AppHandle,
    mode: crate::calibration::Mode,
) -> Result<crate::calibration::CalibrationView, String> {
    crate::calibration::start(&app, mode)
}

#[tauri::command]
pub async fn stop_calibration(app: AppHandle) -> Option<crate::calibration::CalibrationView> {
    crate::calibration::stop(&app);
    crate::calibration::view(&app)
}

#[tauri::command]
pub async fn apply_calibration(app: AppHandle) -> Result<i32, String> {
    crate::calibration::apply_suggestion(&app)
}

#[tauri::command]
pub async fn get_calibration(app: AppHandle) -> Option<crate::calibration::CalibrationView> {
    crate::calibration::view(&app)
}

/// The overlay has applied and drawn the clock anchor `seq` (sent for seeks only).
#[tauri::command]
pub async fn clock_applied(app: AppHandle, seq: u64) {
    app_state::clock_applied(&app, seq);
}

#[tauri::command]
pub async fn list_audio_devices(app: AppHandle) -> Vec<crate::audio_devices::AudioDevice> {
    crate::audio_devices::list(&app)
}

#[tauri::command]
pub async fn set_device_offset(app: AppHandle, id: String, offset_ms: i32) -> Vec<crate::audio_devices::AudioDevice> {
    crate::audio_devices::set_offset(&app, &id, offset_ms);
    crate::audio_devices::list(&app)
}

/// Sets the offset of the current song, e.g. back to 0 from the settings.
#[tauri::command]
pub async fn set_song_offset(app: AppHandle, offset_ms: i32) -> Option<i32> {
    app_state::set_song_offset(&app, offset_ms)
}

#[tauri::command]
pub async fn reload_lyrics(app: AppHandle) {
    app_state::reload_lyrics(&app);
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

/// What became of each configured shortcut (registered, off, taken by another program …).
#[tauri::command]
pub fn hotkey_status(app: AppHandle) -> Vec<crate::hotkeys::HotkeyStatus> {
    crate::hotkeys::status(&app)
}

/// Pauses all shortcuts while the hotkeys page records a new combination.
#[tauri::command]
pub async fn suspend_hotkeys(app: AppHandle, suspended: bool) -> Vec<crate::hotkeys::HotkeyStatus> {
    crate::hotkeys::suspend(&app, suspended);
    crate::hotkeys::status(&app)
}

/// Installed font families and whether a Japanese UI font is there (PLAN.md 9.5).
#[tauri::command]
pub async fn list_fonts() -> crate::fonts::FontList {
    tauri::async_runtime::spawn_blocking(crate::fonts::list)
        .await
        .unwrap_or_else(|_| crate::fonts::FontList { families: Vec::new(), japanese_ui_font: false })
}

/// Sample lines in the overlay while the appearance page is open (PLAN.md 9.5: live preview).
#[tauri::command]
pub async fn set_overlay_preview(app: AppHandle, on: bool) {
    overlay::set_preview(&app, on);
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
