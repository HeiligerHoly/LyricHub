//! Global shortcuts (PLAN.md 9.7). Ctrl+Alt+L toggles the overlay, Ctrl+Alt+E starts the edit
//! mode (pressed again it saves). Enter and Esc are registered only while the edit mode runs:
//! the overlay never holds the keyboard focus, so it cannot receive them any other way.

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::overlay;

fn toggle() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyL)
}
fn edit() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyE)
}
fn save() -> Shortcut {
    Shortcut::new(None, Code::Enter)
}
fn cancel() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let (app, shortcut) = (app.clone(), *shortcut);
            // Off the event-loop thread: the actions save files and (un)register shortcuts.
            std::thread::spawn(move || dispatch(&app, &shortcut));
        })
        .build()
}

fn dispatch(app: &AppHandle, shortcut: &Shortcut) {
    if *shortcut == toggle() {
        overlay::toggle(app);
    } else if *shortcut == edit() {
        if app.state::<overlay::OverlayState>().is_editing() {
            overlay::save_edit(app, None);
        } else {
            overlay::enter_edit(app);
        }
    } else if *shortcut == save() {
        overlay::save_edit(app, None);
    } else if *shortcut == cancel() {
        overlay::cancel_edit(app);
    }
}

pub fn register_defaults(app: &AppHandle) {
    for (name, s) in [("Ctrl+Alt+L", toggle()), ("Ctrl+Alt+E", edit())] {
        if let Err(e) = app.global_shortcut().register(s) {
            // Another app may own the combination; the tray still works.
            tracing::warn!(shortcut = name, error = %e, "could not register global shortcut");
        }
    }
}

/// Registers Enter/Esc for the edit mode, or releases them again.
pub fn set_edit_keys(app: &AppHandle, active: bool) {
    let gs = app.global_shortcut();
    for (name, s) in [("Enter", save()), ("Esc", cancel())] {
        let result = if active {
            if gs.is_registered(s) { Ok(()) } else { gs.register(s) }
        } else if gs.is_registered(s) {
            gs.unregister(s)
        } else {
            Ok(())
        };
        if let Err(e) = result {
            // The buttons in the edit toolbar still work.
            tracing::warn!(key = name, active, error = %e, "could not change edit-mode shortcut");
        }
    }
}
