//! Interface language of what the backend draws itself: the tray menu, notices and the errors
//! that reach the settings window (PLAN.md 9.7, "Sprache"). The windows translate their own
//! texts (src/lib/shared/i18n.svelte.ts). The self-test report stays German: it is a developer
//! diagnostic, matched by the spike scripts.

use tauri::{AppHandle, Manager};
use windows::Win32::Globalization::GetUserDefaultUILanguage;

use crate::settings::{Language, SettingsState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
}

/// The language from the settings; "system" follows the Windows display language.
pub fn current(app: &AppHandle) -> Lang {
    resolve(app.state::<SettingsState>().get().behavior.language)
}

pub fn resolve(language: Language) -> Lang {
    match language {
        Language::De => Lang::De,
        Language::En => Lang::En,
        Language::System => system(),
    }
}

/// German for a German Windows display language, English for everything else.
fn system() -> Lang {
    const LANG_GERMAN: u16 = 0x07;
    let langid = unsafe { GetUserDefaultUILanguage() };
    if langid & 0x3ff == LANG_GERMAN { Lang::De } else { Lang::En }
}

/// Picks the text for `lang`.
pub fn pick(lang: Lang, de: &'static str, en: &'static str) -> &'static str {
    match lang {
        Lang::De => de,
        Lang::En => en,
    }
}
