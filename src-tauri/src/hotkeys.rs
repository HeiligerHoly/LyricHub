//! Global shortcuts (PLAN.md 5.7, 9.7). The combinations come from the settings (`hotkeys`) and
//! can be changed or turned off on the settings page "Hotkeys"; the defaults are Ctrl+Alt+L
//! (overlay on/off), Ctrl+Alt+E (edit mode, pressed again it saves), Ctrl+Alt+K (layout),
//! Ctrl+Alt+R (romaji), Ctrl+Alt+, and Ctrl+Alt+. (song offset −/+100 ms, with Shift ±25 ms).
//! Enter and Esc are registered only while the edit mode runs, Space and Esc only while a
//! calibration or sync protocol runs: the overlay never holds the keyboard focus, so it cannot
//! receive them any other way. Those three are fixed.

use std::str::FromStr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::Serialize;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

use crate::overlay;
use crate::settings::{HotkeySettings, SettingsState};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Toggle,
    Edit,
    Layout,
    Romaji,
    NudgeEarlier,
    NudgeLater,
    NudgeEarlierFine,
    NudgeLaterFine,
}

impl Action {
    pub const ALL: [Action; 8] = [
        Action::Toggle,
        Action::Edit,
        Action::Layout,
        Action::Romaji,
        Action::NudgeEarlier,
        Action::NudgeLater,
        Action::NudgeEarlierFine,
        Action::NudgeLaterFine,
    ];

    fn combo(self, h: &HotkeySettings) -> &str {
        match self {
            Action::Toggle => &h.toggle,
            Action::Edit => &h.edit,
            Action::Layout => &h.layout,
            Action::Romaji => &h.romaji,
            Action::NudgeEarlier => &h.nudge_earlier,
            Action::NudgeLater => &h.nudge_later,
            Action::NudgeEarlierFine => &h.nudge_earlier_fine,
            Action::NudgeLaterFine => &h.nudge_later_fine,
        }
    }
}

/// What became of one configured combination.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct HotkeyStatus {
    pub action: Action,
    pub combo: String,
    /// "ok", "off", "invalid" (not a valid combination), "duplicate" (another action has
    /// it), "reserved" (Enter, Esc or Space alone) or "taken" (another program owns it).
    pub state: &'static str,
}

#[derive(Default)]
pub struct HotkeyState {
    registered: Mutex<Vec<(Action, Shortcut)>>,
    status: Mutex<Vec<HotkeyStatus>>,
    /// While the settings page records a new combination, nothing fires.
    suspended: AtomicBool,
}

fn save() -> Shortcut {
    Shortcut::new(None, Code::Enter)
}
fn cancel() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}
fn tap() -> Shortcut {
    Shortcut::new(None, Code::Space)
}

/// Parses "Ctrl+Alt+L" and friends. Needs a modifier, except for function keys, so a
/// global shortcut never swallows plain typing.
pub fn parse(combo: &str) -> Option<Shortcut> {
    let s = Shortcut::from_str(combo.trim()).ok()?;
    let function_key = matches!(
        s.key,
        Code::F1
            | Code::F2
            | Code::F3
            | Code::F4
            | Code::F5
            | Code::F6
            | Code::F7
            | Code::F8
            | Code::F9
            | Code::F10
            | Code::F11
            | Code::F12
    );
    (!s.mods.is_empty() || function_key).then_some(s)
}

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            // Taps are timed here, before any thread hop (calibration.rs).
            let at = Instant::now();
            let (app, shortcut) = (app.clone(), *shortcut);
            // Off the event-loop thread: the actions save files and (un)register shortcuts.
            std::thread::spawn(move || dispatch(&app, &shortcut, at));
        })
        .build()
}

fn dispatch(app: &AppHandle, shortcut: &Shortcut, at: Instant) {
    let state = app.state::<HotkeyState>();
    if state.suspended.load(Ordering::SeqCst) {
        return;
    }
    if *shortcut == tap() {
        crate::calibration::tap(app, at);
        return;
    } else if *shortcut == save() {
        overlay::save_edit(app, None);
        return;
    } else if *shortcut == cancel() {
        if app.state::<overlay::OverlayState>().is_editing() {
            overlay::cancel_edit(app);
        } else {
            crate::calibration::stop(app);
        }
        return;
    }
    let action = state.registered.lock().unwrap().iter().find(|(_, s)| s == shortcut).map(|(a, _)| *a);
    match action {
        Some(Action::Toggle) => {
            overlay::toggle(app);
        }
        Some(Action::Edit) => {
            if app.state::<overlay::OverlayState>().is_editing() {
                overlay::save_edit(app, None);
            } else {
                overlay::enter_edit(app);
            }
        }
        Some(Action::Layout) => {
            crate::ipc::cycle_layout(app);
        }
        Some(Action::Romaji) => {
            crate::ipc::toggle_romaji(app);
        }
        Some(Action::NudgeEarlier) => {
            crate::ipc::nudge(app, -100);
        }
        Some(Action::NudgeLater) => {
            crate::ipc::nudge(app, 100);
        }
        Some(Action::NudgeEarlierFine) => {
            crate::ipc::nudge(app, -25);
        }
        Some(Action::NudgeLaterFine) => {
            crate::ipc::nudge(app, 25);
        }
        None => {}
    }
}

/// Registers the combinations from the settings, replacing the previous ones. A combination
/// that another program owns is skipped; the tray and the settings still work.
pub fn apply_settings(app: &AppHandle) {
    let hotkeys = app.state::<SettingsState>().get().hotkeys;
    let state = app.state::<HotkeyState>();
    let gs = app.global_shortcut();
    for (_, s) in state.registered.lock().unwrap().drain(..) {
        let _ = gs.unregister(s);
    }
    let mut registered = Vec::new();
    let mut status = Vec::new();
    for action in Action::ALL {
        let combo = action.combo(&hotkeys).trim().to_owned();
        let state_text = if combo.is_empty() {
            "off"
        } else {
            match parse(&combo) {
                None => "invalid",
                Some(s) if s == save() || s == cancel() || s == tap() => "reserved",
                Some(s) if registered.iter().any(|(_, r)| *r == s) => "duplicate",
                Some(s) => match gs.register(s) {
                    Ok(()) => {
                        registered.push((action, s));
                        "ok"
                    }
                    Err(e) => {
                        tracing::warn!(?action, combo = %combo, error = %e, "could not register global shortcut");
                        "taken"
                    }
                },
            }
        };
        status.push(HotkeyStatus { action, combo, state: state_text });
    }
    tracing::info!(registered = registered.len(), "global shortcuts registered");
    *state.registered.lock().unwrap() = registered;
    *state.status.lock().unwrap() = status;
}

pub fn status(app: &AppHandle) -> Vec<HotkeyStatus> {
    app.state::<HotkeyState>().status.lock().unwrap().clone()
}

/// The settings page records a new combination: no action may fire meanwhile, and the old
/// combinations must not swallow the keys. Resuming registers from the settings again.
pub fn suspend(app: &AppHandle, suspended: bool) {
    let state = app.state::<HotkeyState>();
    if state.suspended.swap(suspended, Ordering::SeqCst) == suspended {
        return;
    }
    if suspended {
        let gs = app.global_shortcut();
        for (_, s) in state.registered.lock().unwrap().drain(..) {
            let _ = gs.unregister(s);
        }
    } else {
        apply_settings(app);
    }
}

/// Registers Space (tap) and Esc (stop) while a calibration or sync protocol runs.
pub fn set_tap_keys(app: &AppHandle, active: bool) {
    set_contextual(app, &[("Space", tap()), ("Esc", cancel())], active);
}

/// Registers Enter/Esc for the edit mode, or releases them again.
pub fn set_edit_keys(app: &AppHandle, active: bool) {
    set_contextual(app, &[("Enter", save()), ("Esc", cancel())], active);
}

fn set_contextual(app: &AppHandle, keys: &[(&str, Shortcut)], active: bool) {
    let gs = app.global_shortcut();
    for &(name, s) in keys {
        let result = if active {
            if gs.is_registered(s) { Ok(()) } else { gs.register(s) }
        } else if gs.is_registered(s) {
            gs.unregister(s)
        } else {
            Ok(())
        };
        if let Err(e) = result {
            // The buttons in the edit toolbar and the settings still work.
            tracing::warn!(key = name, active, error = %e, "could not change contextual shortcut");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::Modifiers;

    #[test]
    fn combinations_need_a_modifier_or_a_function_key() {
        assert_eq!(parse("Ctrl+Alt+L"), Some(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyL)));
        assert_eq!(
            parse("Ctrl+Alt+Shift+,"),
            Some(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT), Code::Comma))
        );
        assert!(parse("F9").is_some());
        assert!(parse("L").is_none());
        assert!(parse("Space").is_none());
        assert!(parse("Ctrl+").is_none());
        assert!(parse("Ctrl+Alt+Nope").is_none());
    }

    #[test]
    fn default_combinations_parse() {
        let h = HotkeySettings::default();
        for action in Action::ALL {
            assert!(parse(action.combo(&h)).is_some(), "{action:?}");
        }
    }
}
