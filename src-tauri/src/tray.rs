//! Tray icon (PLAN.md 9.8). Phase 0: overlay toggle, click-through test, self-test, settings, quit.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

use crate::overlay;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "overlay_toggle", "Overlay ein/aus (Strg+Alt+L)", true, None::<&str>)?;
    let click = MenuItem::with_id(app, "overlay_click_through", "Klickdurchlässig umschalten (Strg+Alt+E)", true, None::<&str>)?;
    let selftest = MenuItem::with_id(app, "overlay_selftest", "Overlay-Selbsttest ausführen", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "open_settings", "Einstellungen öffnen", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&toggle, &click, &selftest, &separator, &settings, &quit])?;

    let mut builder = TrayIconBuilder::with_id("lyrichub")
        .tooltip("LyricHub")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "overlay_toggle" => {
                overlay::toggle(app);
            }
            "overlay_click_through" => {
                overlay::toggle_click_through(app);
            }
            "overlay_selftest" => {
                let app = app.clone();
                std::thread::spawn(move || {
                    overlay::selftest::run_and_record(&app);
                });
            }
            "open_settings" => crate::open_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                overlay::toggle(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
