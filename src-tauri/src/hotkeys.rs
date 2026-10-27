//! Global shortcuts (PLAN.md 5.7, 9.7). The combinations come from the settings (`hotkeys`) and
//! can be changed or turned off on the settings page "Hotkeys"; the defaults are Ctrl+Alt+L
//! (overlay on/off), Ctrl+Alt+E (edit mode, pressed again it saves), Ctrl+Alt+K (layout),
//! Ctrl+Alt+R (romaji), Ctrl+Alt+, and Ctrl+Alt+. (song offset −/+100 ms, with Shift ±25 ms).
//! Enter and Esc are registered only while the edit mode runs, Space and Esc only while a
//! calibration or sync protocol runs: the overlay never holds the keyboard focus, so it cannot
//! receive them any other way. Those three are fixed.
//!
//! AltGr reaches Windows as Ctrl+Alt, so a Ctrl+Alt combination also fires while someone types
//! a character with AltGr (AltGr+E is € on a German keyboard, AltGr+L is ł on a Polish one),
//! and Windows swallows the key. Such a press is handed back to the window in front instead
//! (`altgr_typing`, `pass_through`).

use std::str::FromStr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, HKL, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC_EX, MapVirtualKeyExW, SendInput, ToUnicodeEx, VIRTUAL_KEY, VK_CONTROL,
    VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_OEM_1, VK_OEM_2, VK_OEM_3, VK_OEM_4, VK_OEM_5, VK_OEM_6, VK_OEM_7,
    VK_OEM_COMMA, VK_OEM_MINUS, VK_OEM_PERIOD, VK_OEM_PLUS, VK_RMENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

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
            // Also here, while the keys are still held.
            let typing = altgr_typing(shortcut);
            let (app, shortcut) = (app.clone(), *shortcut);
            // Off the event-loop thread: the actions save files and (un)register shortcuts.
            std::thread::spawn(move || {
                if typing {
                    pass_through(&app, shortcut);
                } else {
                    dispatch(&app, &shortcut, at);
                }
            });
        })
        .build()
}

/// True when a Ctrl+Alt combination fired because someone types a character with AltGr:
/// right Alt is held without left Alt, and AltGr+key types something (a dead key counts) in
/// the layout of the window in front. Ctrl+Alt with the left Alt key stays a shortcut, and so
/// does an AltGr combination that types nothing.
fn altgr_typing(shortcut: &Shortcut) -> bool {
    if !shortcut.mods.contains(Modifiers::CONTROL | Modifiers::ALT)
        || shortcut.mods.intersects(Modifiers::SUPER | Modifiers::META)
    {
        return false;
    }
    let Some(vk) = typing_vk(shortcut.key) else {
        return false;
    };
    key_down(VK_RMENU)
        && !key_down(VK_LMENU)
        && altgr_types(vk, shortcut.mods.contains(Modifiers::SHIFT), foreground_layout())
}

/// Hands a swallowed AltGr key press back: unregisters the combination, replays the key (the
/// AltGr still held turns it into €) and registers the combination again once the key is up,
/// so holding the key repeats like normal typing.
fn pass_through(app: &AppHandle, shortcut: Shortcut) {
    let Some(vk) = typing_vk(shortcut.key) else {
        return;
    };
    let gs = app.global_shortcut();
    if let Err(e) = gs.unregister(shortcut) {
        // Replaying the key would only fire the combination again.
        tracing::warn!(error = %e, "could not release a shortcut for an AltGr key");
        return;
    }
    replay_key(vk, !key_down(vk));
    let since = Instant::now();
    while key_down(vk) && since.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let state = app.state::<HotkeyState>();
    // Held while registering, so `apply_settings` cannot drain the list in between. If the
    // settings changed meanwhile, the combination is gone from the list or registered again.
    let registered = state.registered.lock().unwrap();
    if !state.suspended.load(Ordering::SeqCst)
        && registered.iter().any(|(_, s)| *s == shortcut)
        && !gs.is_registered(shortcut)
        && let Err(e) = gs.register(shortcut)
    {
        tracing::warn!(error = %e, "could not register a shortcut again after an AltGr key");
    }
}

/// The virtual key that global-hotkey registers for a key that can type a character
/// (letters, digits, punctuation); None for the rest (function keys, arrows, numpad).
fn typing_vk(key: Code) -> Option<VIRTUAL_KEY> {
    use Code::*;
    const LETTERS: [Code; 26] = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR,
        KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    ];
    const DIGITS: [Code; 10] = [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
    if let Some(i) = LETTERS.iter().position(|&c| c == key) {
        return Some(VIRTUAL_KEY(b'A' as u16 + i as u16));
    }
    if let Some(i) = DIGITS.iter().position(|&c| c == key) {
        return Some(VIRTUAL_KEY(b'0' as u16 + i as u16));
    }
    Some(match key {
        Equal => VK_OEM_PLUS,
        Comma => VK_OEM_COMMA,
        Minus => VK_OEM_MINUS,
        Period => VK_OEM_PERIOD,
        Semicolon => VK_OEM_1,
        Slash => VK_OEM_2,
        Backquote => VK_OEM_3,
        BracketLeft => VK_OEM_4,
        Backslash => VK_OEM_5,
        BracketRight => VK_OEM_6,
        Quote => VK_OEM_7,
        _ => return None,
    })
}

fn key_down(vk: VIRTUAL_KEY) -> bool {
    // SAFETY: plain query; the high bit means the key is down right now.
    unsafe { GetAsyncKeyState(vk.0 as i32) < 0 }
}

fn foreground_layout() -> HKL {
    // SAFETY: plain queries; with no window in front, thread 0 gives this thread's layout.
    unsafe { GetKeyboardLayout(GetWindowThreadProcessId(GetForegroundWindow(), None)) }
}

/// Whether Ctrl+Alt (= AltGr) plus `vk`, with or without Shift, types something in `layout`.
fn altgr_types(vk: VIRTUAL_KEY, shift: bool, layout: HKL) -> bool {
    let mut keys = [0u8; 256];
    for k in [VK_CONTROL, VK_LCONTROL, VK_MENU, VK_RMENU] {
        keys[k.0 as usize] = 0x80;
    }
    if shift {
        keys[VK_SHIFT.0 as usize] = 0x80;
        keys[VK_LSHIFT.0 as usize] = 0x80;
    }
    let mut text = [0u16; 8];
    // SAFETY: both buffers are local. Flag 4 leaves the keyboard state (dead keys) untouched;
    // a negative result is a dead key.
    unsafe { ToUnicodeEx(vk.0 as u32, 0, &keys, &mut text, 4, Some(layout)) != 0 }
}

/// Sends `vk` down, and up as well when the real key is already up (its release would
/// otherwise never come); while it is held, the real release follows by itself.
fn replay_key(vk: VIRTUAL_KEY, with_release: bool) {
    // SAFETY: plain query.
    let scan = unsafe { MapVirtualKeyExW(vk.0 as u32, MAPVK_VK_TO_VSC_EX, Some(foreground_layout())) } as u16;
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    };
    let inputs = if with_release {
        vec![key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)]
    } else {
        vec![key(KEYBD_EVENT_FLAGS(0))]
    };
    // SAFETY: the slice holds initialised keyboard inputs of the size passed.
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if (sent as usize) < inputs.len() {
        // For example when the window in front runs as administrator.
        tracing::warn!("could not replay an AltGr key");
    }
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
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayoutList, VK_E, VK_K, VK_L, VK_Q, VK_R};

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

    #[test]
    fn typing_keys_map_to_the_registered_virtual_keys() {
        assert_eq!(typing_vk(Code::KeyE), Some(VK_E));
        assert_eq!(typing_vk(Code::KeyZ), Some(VIRTUAL_KEY(b'Z' as u16)));
        assert_eq!(typing_vk(Code::Digit7), Some(VIRTUAL_KEY(b'7' as u16)));
        assert_eq!(typing_vk(Code::Comma), Some(VK_OEM_COMMA));
        assert_eq!(typing_vk(Code::F9), None);
        assert_eq!(typing_vk(Code::Enter), None);
    }

    /// Reads the German layout if Windows has it installed; changes nothing.
    #[test]
    fn altgr_types_euro_but_not_the_other_default_keys_on_a_german_layout() {
        let mut layouts = [HKL::default(); 32];
        // SAFETY: the buffer is local.
        let n = unsafe { GetKeyboardLayoutList(Some(&mut layouts)) } as usize;
        let Some(&german) = layouts[..n].iter().find(|l| l.0 as usize & 0xFFFF == 0x0407) else {
            eprintln!("no German keyboard layout installed, skipped");
            return;
        };
        assert!(altgr_types(VK_E, false, german), "AltGr+E is €");
        assert!(altgr_types(VK_Q, false, german), "AltGr+Q is @");
        for vk in [VK_L, VK_K, VK_R, VK_OEM_COMMA, VK_OEM_PERIOD] {
            assert!(!altgr_types(vk, false, german), "{vk:?}");
        }
        assert!(!altgr_types(VK_OEM_COMMA, true, german));
    }
}
