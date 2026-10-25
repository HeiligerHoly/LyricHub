//! IPC between backend and windows (PLAN.md 4.4): commands, event names, and the one place
//! that applies a settings change.

pub mod commands;
pub mod events;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::settings::{OFFSET_LIMIT_MS, Settings, SettingsState};

/// Emits the new settings and applies what changed. Every settings change ends here.
pub fn settings_changed(app: &AppHandle, before: &Settings, after: &Settings) {
    if before == after {
        return;
    }
    if let Err(e) = app.emit(events::SETTINGS, after) {
        tracing::warn!(error = %e, "could not emit settings");
    }
    if before.overlay != after.overlay {
        crate::overlay::relayout(app);
    }
    if before.overlay.layout != after.overlay.layout {
        crate::tray::sync_layout(app, after.overlay.layout);
    }
    if before.sync != after.sync {
        crate::app_state::emit_clock(app);
    }
    if before.behavior.autostart != after.behavior.autostart {
        apply_autostart(app, after.behavior.autostart);
    }
}

#[derive(Serialize, Clone, Copy)]
struct NudgeEvent {
    offset_ms: i32,
    delta_ms: i32,
}

/// Nudge hotkeys (PLAN.md 5.7): shifts the global offset; positive shows the lyrics later.
/// The song offset as default target follows with Phase 3.
pub fn nudge(app: &AppHandle, delta_ms: i32) -> Settings {
    let (before, after) = app.state::<SettingsState>().update(|s| {
        s.sync.global_offset_ms = (s.sync.global_offset_ms + delta_ms).clamp(-OFFSET_LIMIT_MS, OFFSET_LIMIT_MS);
    });
    tracing::info!(delta_ms, offset_ms = after.sync.global_offset_ms, "offset nudged");
    settings_changed(app, &before, &after);
    let event = NudgeEvent { offset_ms: after.sync.global_offset_ms, delta_ms };
    if let Err(e) = app.emit(events::NUDGE, event) {
        tracing::warn!(error = %e, "could not emit nudge");
    }
    after
}

/// Switches to the next overlay layout (Ctrl+Alt+K, tray).
pub fn cycle_layout(app: &AppHandle) -> Settings {
    let (before, after) = app.state::<SettingsState>().update(|s| s.overlay.layout = s.overlay.layout.next());
    tracing::info!(layout = ?after.overlay.layout, "layout switched");
    settings_changed(app, &before, &after);
    after
}

/// Makes the Windows autostart entry match the setting. The setting is the user's intent;
/// the registry entry is repaired at each start (e.g. after the exe moved).
pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    let autolaunch = app.autolaunch();
    if autolaunch.is_enabled().ok() == Some(enabled) {
        return;
    }
    let result = if enabled { autolaunch.enable() } else { autolaunch.disable() };
    match result {
        Ok(()) => tracing::info!(enabled, "autostart updated"),
        Err(e) => tracing::warn!(enabled, error = %e, "could not update autostart"),
    }
}
