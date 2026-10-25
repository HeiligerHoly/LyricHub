//! IPC between backend and windows (PLAN.md 4.4): commands, event names, and the one place
//! that applies a settings change.

pub mod commands;
pub mod events;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::settings::{Settings, SettingsState};

/// Emits the new settings and applies what changed. Every settings change ends here.
pub fn settings_changed(app: &AppHandle, before: &Settings, after: &Settings) {
    if before == after {
        return;
    }
    if before.appearance.romaji != after.appearance.romaji {
        // Before the settings event, so the overlay has the romaji when it switches.
        crate::app_state::romaji_changed(app, after.appearance.romaji);
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
        crate::app_state::emit_clock(app, "offset");
    }
    if before.behavior.autostart != after.behavior.autostart {
        apply_autostart(app, after.behavior.autostart);
    }
}

#[derive(Serialize, Clone, Copy)]
struct NudgeEvent {
    /// New offset of the current song.
    offset_ms: i32,
    delta_ms: i32,
}

/// Nudge hotkeys (PLAN.md 5.7, user decision 14.1): change the offset of the song that plays
/// and store it for that song; positive shows the lyrics later. The global offset changes only
/// in the settings. Returns the new song offset, `None` when nothing plays.
pub fn nudge(app: &AppHandle, delta_ms: i32) -> Option<i32> {
    let offset_ms = crate::app_state::nudge_song(app, delta_ms)?;
    if let Err(e) = app.emit(events::NUDGE, NudgeEvent { offset_ms, delta_ms }) {
        tracing::warn!(error = %e, "could not emit nudge");
    }
    Some(offset_ms)
}

/// Switches to the next overlay layout (Ctrl+Alt+K, tray).
pub fn cycle_layout(app: &AppHandle) -> Settings {
    let (before, after) = app.state::<SettingsState>().update(|s| s.overlay.layout = s.overlay.layout.next());
    tracing::info!(layout = ?after.overlay.layout, "layout switched");
    settings_changed(app, &before, &after);
    after
}

/// Shows Japanese lines in romaji or in the original script (Ctrl+Alt+R).
pub fn toggle_romaji(app: &AppHandle) -> Settings {
    let (before, after) = app.state::<SettingsState>().update(|s| s.appearance.romaji = !s.appearance.romaji);
    tracing::info!(on = after.appearance.romaji, "romaji toggled");
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
