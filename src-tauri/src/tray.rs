//! Tray icon (PLAN.md 9.8). Left click toggles the overlay; the menu has the overlay toggle,
//! the edit mode, nudge −/+100 ms, the layout, the settings, the self-test and quit. The
//! tooltip names the current song. Texts follow the interface language (i18n.rs).

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::i18n::{self, Lang, pick};
use crate::overlay;
use crate::settings::{Layout, SettingsState};

const TRAY_ID: &str = "lyrichub";
const TOOLTIP_MAX: usize = 120;

pub struct TrayItems {
    overlay_visible: CheckMenuItem<Wry>,
    edit: MenuItem<Wry>,
    editing: std::sync::atomic::AtomicBool,
    nudge_back: MenuItem<Wry>,
    nudge_forward: MenuItem<Wry>,
    layout_menu: Submenu<Wry>,
    settings: MenuItem<Wry>,
    selftest: MenuItem<Wry>,
    quit: MenuItem<Wry>,
    layouts: Vec<(Layout, CheckMenuItem<Wry>)>,
    /// Disabled first entry for a hint (overlay covered, exclusive fullscreen); only in the
    /// menu while there is one.
    menu: Menu<Wry>,
    hint: MenuItem<Wry>,
    hint_text: std::sync::Mutex<Option<String>>,
    /// Song line of the tooltip, kept so a hint can be added to it.
    song_line: std::sync::Mutex<Option<String>>,
    /// Coloured and grey icon; grey while Spotify is not running (PLAN.md 5.5).
    icons: Option<(Image<'static>, Image<'static>)>,
    player_present: std::sync::atomic::AtomicBool,
}

/// Tray variant of the app icon (icons/tray.svg): the same motif with a wider, brighter rim,
/// so the dark half of the tile stays apart from a dark taskbar. Pre-rendered for 100 % to
/// 300 % scaling; Windows would blur a single large bitmap down to 16 px.
const TRAY_ICONS: [(i32, &[u8]); 6] = [
    (16, include_bytes!("../icons/tray/16.png")),
    (20, include_bytes!("../icons/tray/20.png")),
    (24, include_bytes!("../icons/tray/24.png")),
    (32, include_bytes!("../icons/tray/32.png")),
    (40, include_bytes!("../icons/tray/40.png")),
    (48, include_bytes!("../icons/tray/48.png")),
];

/// The tray variant at the size of a small icon for the system DPI, or the next larger one.
fn tray_icon() -> Option<Image<'static>> {
    use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi};
    use windows::Win32::UI::WindowsAndMessaging::SM_CXSMICON;
    // SAFETY: plain system queries.
    let wanted = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) };
    let (size, bytes) = TRAY_ICONS.iter().find(|(s, _)| *s >= wanted).unwrap_or(&TRAY_ICONS[TRAY_ICONS.len() - 1]);
    match Image::from_bytes(bytes) {
        Ok(image) => {
            tracing::debug!(wanted, size, "tray icon size");
            Some(image.to_owned())
        }
        Err(e) => {
            tracing::warn!(error = %e, "tray icon unreadable, using the window icon");
            None
        }
    }
}

/// Grey copy of an RGBA icon (luminance, alpha kept, a little dimmed).
fn greyscale(icon: &Image<'_>) -> Image<'static> {
    let rgba: Vec<u8> = icon
        .rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let l = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) * 0.8;
            let l = l.round().clamp(0.0, 255.0) as u8;
            [l, l, l, p[3]]
        })
        .collect();
    Image::new_owned(rgba, icon.width(), icon.height())
}

fn layout_label(lang: Lang, l: Layout) -> &'static str {
    match l {
        Layout::Karaoke => "Karaoke",
        Layout::Bar => pick(lang, "Leiste", "Bar"),
        Layout::Stack => pick(lang, "Stapel", "Stack"),
        Layout::Minimal => "Minimal",
    }
}

/// " (Strg+Alt+L)" for a configured shortcut (Settings → Hotkeys), nothing when it is off.
fn combo(app: &AppHandle, lang: Lang, pick_combo: impl Fn(&crate::settings::HotkeySettings) -> &str) -> String {
    let hotkeys = app.state::<SettingsState>().get().hotkeys;
    let c = pick_combo(&hotkeys).trim().to_owned();
    if c.is_empty() {
        return String::new();
    }
    match lang {
        Lang::De => format!(" ({})", c.replace("Ctrl", "Strg")),
        Lang::En => format!(" ({c})"),
    }
}

fn edit_label(app: &AppHandle, lang: Lang, editing: bool) -> String {
    if editing {
        pick(lang, "Edit-Modus speichern (Enter)", "Save edit mode (Enter)").to_owned()
    } else {
        pick(lang, "Position und Größe ändern", "Change position and size").to_owned() + &combo(app, lang, |h| &h.edit)
    }
}

struct Texts {
    visible: String,
    nudge_back: String,
    nudge_forward: String,
    layout: String,
    settings: &'static str,
    selftest: &'static str,
    quit: &'static str,
}

fn texts(app: &AppHandle, lang: Lang) -> Texts {
    Texts {
        visible: pick(lang, "Overlay anzeigen", "Show overlay").to_owned() + &combo(app, lang, |h| &h.toggle),
        nudge_back: pick(lang, "Song: Lyrics früher, −100 ms", "Song: lyrics earlier, −100 ms").to_owned()
            + &combo(app, lang, |h| &h.nudge_earlier),
        nudge_forward: pick(lang, "Song: Lyrics später, +100 ms", "Song: lyrics later, +100 ms").to_owned()
            + &combo(app, lang, |h| &h.nudge_later),
        layout: "Layout".to_owned() + &combo(app, lang, |h| &h.layout),
        settings: pick(lang, "Einstellungen …", "Settings …"),
        selftest: pick(lang, "Overlay-Selbsttest ausführen", "Run overlay self-test"),
        quit: pick(lang, "Beenden", "Quit"),
    }
}

fn layout_id(l: Layout) -> &'static str {
    match l {
        Layout::Karaoke => "layout_karaoke",
        Layout::Bar => "layout_bar",
        Layout::Stack => "layout_stack",
        Layout::Minimal => "layout_minimal",
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let lang = i18n::current(app);
    let t = texts(app, lang);
    let visible = CheckMenuItem::with_id(app, "overlay_visible", &t.visible, true, true, None::<&str>)?;
    let edit = MenuItem::with_id(app, "edit_mode", edit_label(app, lang, false), true, None::<&str>)?;
    let nudge_back = MenuItem::with_id(app, "nudge_minus", &t.nudge_back, true, None::<&str>)?;
    let nudge_forward = MenuItem::with_id(app, "nudge_plus", &t.nudge_forward, true, None::<&str>)?;
    let current = app.state::<SettingsState>().get().overlay.layout;
    let mut layouts = Vec::new();
    for l in Layout::ALL {
        layouts.push((
            l,
            CheckMenuItem::with_id(app, layout_id(l), layout_label(lang, l), true, l == current, None::<&str>)?,
        ));
    }
    let layout_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> =
        layouts.iter().map(|(_, i)| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
    let layout_menu = Submenu::with_items(app, &t.layout, true, &layout_refs)?;
    let settings = MenuItem::with_id(app, "open_settings", t.settings, true, None::<&str>)?;
    let selftest = MenuItem::with_id(app, "overlay_selftest", t.selftest, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", t.quit, true, None::<&str>)?;
    let hint = MenuItem::with_id(app, "hint", "", false, None::<&str>)?;
    let separator = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(
        app,
        &[
            &visible,
            &edit,
            &separator()?,
            &nudge_back,
            &nudge_forward,
            &layout_menu,
            &separator()?,
            &settings,
            &selftest,
            &separator()?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("LyricHub")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let app = app.clone();
            let id = event.id().as_ref().to_owned();
            // Off the event-loop thread, like the hotkeys: actions may wait for the main thread.
            std::thread::spawn(move || match id.as_str() {
                "overlay_visible" => {
                    overlay::toggle(&app);
                }
                "edit_mode" => {
                    if app.state::<overlay::OverlayState>().is_editing() {
                        overlay::save_edit(&app, None);
                    } else {
                        overlay::enter_edit(&app);
                    }
                }
                "overlay_selftest" => {
                    overlay::selftest::run_and_record(&app);
                }
                "nudge_minus" => {
                    crate::ipc::nudge(&app, -100);
                }
                "nudge_plus" => {
                    crate::ipc::nudge(&app, 100);
                }
                id if id.starts_with("layout_") => {
                    let pick = Layout::ALL.into_iter().find(|l| layout_id(*l) == id);
                    if let Some(layout) = pick {
                        let (before, after) = app.state::<SettingsState>().update(|s| s.overlay.layout = layout);
                        crate::ipc::settings_changed(&app, &before, &after);
                    }
                }
                "open_settings" => crate::settings_window::open(&app),
                "quit" => app.exit(0),
                _ => {}
            });
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let app = tray.app_handle().clone();
                std::thread::spawn(move || {
                    overlay::toggle(&app);
                });
            }
        });
    let icon = tray_icon().or_else(|| app.default_window_icon().map(|i| i.clone().to_owned()));
    if let Some(icon) = &icon {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    let icons = icon.map(|i| {
        let grey = greyscale(&i);
        (i, grey)
    });
    app.manage(TrayItems {
        overlay_visible: visible,
        edit,
        editing: false.into(),
        nudge_back,
        nudge_forward,
        layout_menu,
        settings,
        selftest,
        quit,
        layouts,
        menu,
        hint,
        hint_text: None.into(),
        song_line: None.into(),
        icons,
        player_present: true.into(),
    });
    Ok(())
}

/// Grey tray icon while no Spotify session exists, coloured otherwise.
pub fn set_player_present(app: &AppHandle, present: bool) {
    let Some(items) = app.try_state::<TrayItems>() else { return };
    if items.player_present.swap(present, std::sync::atomic::Ordering::SeqCst) == present {
        return;
    }
    if let (Some((colour, grey)), Some(tray)) = (&items.icons, app.tray_by_id(TRAY_ID)) {
        let _ = tray.set_icon(Some(if present { colour.clone() } else { grey.clone() }));
    }
}

/// Keeps the layout check marks in line with the settings.
pub fn sync_layout(app: &AppHandle, current: Layout) {
    if let Some(items) = app.try_state::<TrayItems>() {
        for (l, item) in &items.layouts {
            let _ = item.set_checked(*l == current);
        }
    }
}

pub fn sync_overlay_visible(app: &AppHandle, visible: bool) {
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.overlay_visible.set_checked(visible);
    }
}

pub fn sync_edit_mode(app: &AppHandle, editing: bool) {
    if let Some(items) = app.try_state::<TrayItems>() {
        items.editing.store(editing, std::sync::atomic::Ordering::SeqCst);
        let _ = items.edit.set_text(edit_label(app, i18n::current(app), editing));
    }
}

/// Writes every menu text again: interface language and shortcuts may have changed.
pub fn apply_language(app: &AppHandle) {
    let Some(items) = app.try_state::<TrayItems>() else { return };
    let lang = i18n::current(app);
    let t = texts(app, lang);
    let _ = items.overlay_visible.set_text(&t.visible);
    let _ = items.edit.set_text(edit_label(app, lang, items.editing.load(std::sync::atomic::Ordering::SeqCst)));
    let _ = items.nudge_back.set_text(&t.nudge_back);
    let _ = items.nudge_forward.set_text(&t.nudge_forward);
    let _ = items.layout_menu.set_text(&t.layout);
    let _ = items.settings.set_text(t.settings);
    let _ = items.selftest.set_text(t.selftest);
    let _ = items.quit.set_text(t.quit);
    for (l, item) in &items.layouts {
        let _ = item.set_text(layout_label(lang, *l));
    }
}

/// "LyricHub – Title – Artist", or a status line when nothing plays; a hint goes first.
pub fn set_tooltip(app: &AppHandle, line: Option<&str>) {
    if let Some(items) = app.try_state::<TrayItems>() {
        *items.song_line.lock().unwrap() = line.map(str::to_owned);
    }
    refresh_tooltip(app);
}

/// Shows a hint in the tooltip and as the first, disabled menu entry, or removes it.
pub fn set_hint(app: &AppHandle, hint: Option<String>) {
    let Some(items) = app.try_state::<TrayItems>() else { return };
    let before = std::mem::replace(&mut *items.hint_text.lock().unwrap(), hint.clone());
    if before == hint {
        return;
    }
    match (&before, &hint) {
        (None, Some(text)) => {
            let _ = items.hint.set_text(text);
            let _ = items.menu.insert(&items.hint, 0);
        }
        (Some(_), Some(text)) => {
            let _ = items.hint.set_text(text);
        }
        (Some(_), None) => {
            let _ = items.menu.remove(&items.hint);
        }
        (None, None) => {}
    }
    refresh_tooltip(app);
}

fn refresh_tooltip(app: &AppHandle) {
    let Some(items) = app.try_state::<TrayItems>() else { return };
    let song = items.song_line.lock().unwrap().clone();
    let hint = items.hint_text.lock().unwrap().clone();
    let mut text = match song {
        Some(l) => format!("LyricHub – {l}"),
        None => {
            let browser = crate::sources::smtc::is_browser(&app.state::<SettingsState>().get().player.app_id);
            let idle = match browser {
                false => pick(i18n::current(app), "Spotify spielt nichts", "Spotify is not playing"),
                true => pick(i18n::current(app), "Der Player spielt nichts", "The player is not playing"),
            };
            format!("LyricHub – {idle}")
        }
    };
    if let Some(h) = hint {
        text = format!("{h}\n{text}");
    }
    if text.chars().count() > TOOLTIP_MAX {
        text = text.chars().take(TOOLTIP_MAX - 1).collect::<String>() + "…";
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}
