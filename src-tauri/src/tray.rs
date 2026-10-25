//! Tray icon (PLAN.md 9.8). Left click toggles the overlay; the menu has the overlay toggle,
//! the edit mode, nudge −/+100 ms, the layout, the settings, the self-test and quit. The
//! tooltip names the current song.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::overlay;
use crate::settings::{Layout, SettingsState};

const TRAY_ID: &str = "lyrichub";
const TOOLTIP_MAX: usize = 120;

pub struct TrayItems {
    overlay_visible: CheckMenuItem<Wry>,
    edit: MenuItem<Wry>,
    layouts: Vec<(Layout, CheckMenuItem<Wry>)>,
}

fn layout_label(l: Layout) -> &'static str {
    match l {
        Layout::Karaoke => "Karaoke",
        Layout::Bar => "Leiste",
    }
}

fn layout_id(l: Layout) -> &'static str {
    match l {
        Layout::Karaoke => "layout_karaoke",
        Layout::Bar => "layout_bar",
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let visible = CheckMenuItem::with_id(app, "overlay_visible", "Overlay anzeigen (Strg+Alt+L)", true, true, None::<&str>)?;
    let edit = MenuItem::with_id(app, "edit_mode", "Position und Größe ändern (Strg+Alt+E)", true, None::<&str>)?;
    let nudge_back = MenuItem::with_id(app, "nudge_minus", "Lyrics früher: −100 ms (Strg+Alt+,)", true, None::<&str>)?;
    let nudge_forward = MenuItem::with_id(app, "nudge_plus", "Lyrics später: +100 ms (Strg+Alt+.)", true, None::<&str>)?;
    let current = app.state::<SettingsState>().get().overlay.layout;
    let mut layouts = Vec::new();
    for l in Layout::ALL {
        layouts.push((l, CheckMenuItem::with_id(app, layout_id(l), layout_label(l), true, l == current, None::<&str>)?));
    }
    let layout_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = layouts.iter().map(|(_, i)| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
    let layout_menu = Submenu::with_items(app, "Layout (Strg+Alt+K)", true, &layout_refs)?;
    let settings = MenuItem::with_id(app, "open_settings", "Einstellungen …", true, None::<&str>)?;
    let selftest = MenuItem::with_id(app, "overlay_selftest", "Overlay-Selbsttest ausführen", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
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
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    app.manage(TrayItems { overlay_visible: visible, edit, layouts });
    Ok(())
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
        let text = if editing { "Edit-Modus speichern (Enter)" } else { "Position und Größe ändern (Strg+Alt+E)" };
        let _ = items.edit.set_text(text);
    }
}

/// "LyricHub – Title – Artist", or a status line when nothing plays.
pub fn set_tooltip(app: &AppHandle, line: Option<&str>) {
    let mut text = match line {
        Some(l) => format!("LyricHub – {l}"),
        None => "LyricHub – Spotify spielt nichts".to_owned(),
    };
    if text.chars().count() > TOOLTIP_MAX {
        text = text.chars().take(TOOLTIP_MAX - 1).collect::<String>() + "…";
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}
