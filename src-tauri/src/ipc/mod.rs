//! IPC between backend and windows (PLAN.md 4.4): commands, event names, and the one place
//! that applies a settings change.

pub mod commands;
pub mod events;

use tauri::{AppHandle, Emitter};
use tauri_plugin_autostart::ManagerExt;

use crate::settings::Settings;

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
    if before.behavior.autostart != after.behavior.autostart {
        apply_autostart(app, after.behavior.autostart);
    }
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
