//! Gamepad buttons (IDEEN.md, package D): XInput controllers, read every 100 ms and only read;
//! the game sees the same buttons, so every action needs a held modifier (Back/View, both
//! shoulder buttons or both sticks pressed). With the modifier held: Y shows or hides the overlay,
//! X switches the layout, B the Latin script, the D-pad left and right nudge by 100 ms. Off by
//! default; the actions are the hotkeys' (hotkeys::perform).

use std::time::Duration;

use tauri::{AppHandle, Manager};
use windows::Win32::UI::Input::XboxController::{
    XINPUT_GAMEPAD_B, XINPUT_GAMEPAD_BACK, XINPUT_GAMEPAD_BUTTON_FLAGS, XINPUT_GAMEPAD_DPAD_LEFT, XINPUT_GAMEPAD_DPAD_RIGHT,
    XINPUT_GAMEPAD_LEFT_SHOULDER, XINPUT_GAMEPAD_LEFT_THUMB, XINPUT_GAMEPAD_RIGHT_SHOULDER, XINPUT_GAMEPAD_RIGHT_THUMB, XINPUT_GAMEPAD_X,
    XINPUT_GAMEPAD_Y, XINPUT_STATE, XInputGetState,
};

use crate::hotkeys::Action;
use crate::settings::{GamepadModifier, SettingsState};

const POLL: Duration = Duration::from_millis(100);
const OFF_POLL: Duration = Duration::from_secs(1);
/// Empty slots are slow to ask (XInput's own advice): look again this rarely.
const EMPTY_SLOT_EVERY: u32 = 20;

fn bits(f: XINPUT_GAMEPAD_BUTTON_FLAGS) -> u16 {
    f.0
}

fn modifier_bits(m: GamepadModifier) -> u16 {
    match m {
        GamepadModifier::Back => bits(XINPUT_GAMEPAD_BACK),
        GamepadModifier::Shoulders => bits(XINPUT_GAMEPAD_LEFT_SHOULDER) | bits(XINPUT_GAMEPAD_RIGHT_SHOULDER),
        GamepadModifier::Sticks => bits(XINPUT_GAMEPAD_LEFT_THUMB) | bits(XINPUT_GAMEPAD_RIGHT_THUMB),
    }
}

/// The actions whose button went down between `before` and `now` while the modifier is held.
pub fn actions(before: u16, now: u16, modifier: GamepadModifier) -> Vec<Action> {
    let m = modifier_bits(modifier);
    if now & m != m {
        return Vec::new();
    }
    let pressed = now & !before;
    [
        (XINPUT_GAMEPAD_Y, Action::Toggle),
        (XINPUT_GAMEPAD_X, Action::Layout),
        (XINPUT_GAMEPAD_B, Action::Romaji),
        (XINPUT_GAMEPAD_DPAD_LEFT, Action::NudgeEarlier),
        (XINPUT_GAMEPAD_DPAD_RIGHT, Action::NudgeLater),
    ]
    .into_iter()
    .filter(|(b, _)| pressed & bits(*b) != 0)
    .map(|(_, a)| a)
    .collect()
}

pub fn spawn(app: AppHandle) {
    let _ = std::thread::Builder::new().name("gamepad".into()).spawn(move || run(&app));
}

fn run(app: &AppHandle) {
    let mut before = [0u16; 4];
    let mut connected = [true; 4];
    let mut round = 0u32;
    loop {
        let settings = app.state::<SettingsState>().get().gamepad;
        if !settings.enabled {
            before = [0; 4];
            std::thread::sleep(OFF_POLL);
            continue;
        }
        round = round.wrapping_add(1);
        for slot in 0..4 {
            if !connected[slot] && !round.is_multiple_of(EMPTY_SLOT_EVERY) {
                continue;
            }
            let mut state = XINPUT_STATE::default();
            // SAFETY: XInputGetState writes the state of one slot into a local.
            connected[slot] = unsafe { XInputGetState(slot as u32, &mut state) } == 0;
            let now = if connected[slot] { bits(state.Gamepad.wButtons) } else { 0 };
            for action in actions(before[slot], now, settings.modifier) {
                tracing::info!(action = ?action, "gamepad combination");
                crate::hotkeys::perform(app, action);
            }
            before[slot] = now;
        }
        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_combination_fires_once_while_the_modifier_is_held() {
        let back = bits(XINPUT_GAMEPAD_BACK);
        let y = bits(XINPUT_GAMEPAD_Y);
        assert_eq!(actions(back, back | y, GamepadModifier::Back), [Action::Toggle]);
        assert!(actions(back | y, back | y, GamepadModifier::Back).is_empty(), "held, not pressed again");
        assert!(actions(0, y, GamepadModifier::Back).is_empty(), "without the modifier the game has Y");
        let shoulders = bits(XINPUT_GAMEPAD_LEFT_SHOULDER) | bits(XINPUT_GAMEPAD_RIGHT_SHOULDER);
        let left = bits(XINPUT_GAMEPAD_DPAD_LEFT);
        assert_eq!(actions(shoulders, shoulders | left, GamepadModifier::Shoulders), [Action::NudgeEarlier]);
        assert!(actions(0, bits(XINPUT_GAMEPAD_LEFT_SHOULDER) | left, GamepadModifier::Shoulders).is_empty(), "one shoulder is not enough");
    }
}
