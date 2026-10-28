//! LyricHub backend: SMTC source, playback clock with offsets per song and audio device,
//! lyrics from LRCLIB with cache, Sync Lab, settings store, overlay geometry with edit mode,
//! tray and hotkeys.

mod app_state;
mod audio_devices;
mod calibration;
mod fonts;
mod hotkeys;
mod i18n;
mod ipc;
mod logging;
mod lyrics;
mod overlay;
mod paths;
mod plugins;
mod settings;
mod settings_window;
mod sources;
mod sync;
mod sync_lab;
mod tray;

use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

use sources::PlaybackSource;

pub fn run() {
    logging::init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "LyricHub starting");

    paths::drop_webview_components();
    let autostarted = std::env::args().any(|a| a == "--minimized");
    let selftest_mode = std::env::var("LYRICHUB_SELFTEST").ok();

    let app = tauri::Builder::default()
        // A second start opens the settings of the running instance; a second autostart does nothing.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !args.iter().any(|a| a == "--minimized") {
                settings_window::open(app);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(hotkeys::plugin())
        .manage(overlay::OverlayState::default())
        .manage(hotkeys::HotkeyState::default())
        .manage(settings::SettingsState::load())
        .manage(app_state::NowPlayingState::default())
        .manage(sync_lab::SyncLabState::default())
        .manage(audio_devices::AudioDeviceState::default())
        .manage(calibration::CalibrationState::default())
        .manage(plugins::PluginsState::default())
        .invoke_handler(tauri::generate_handler![
            ipc::commands::get_settings,
            ipc::commands::set_settings,
            ipc::commands::set_overlay_anchor,
            ipc::commands::reset_overlay_geometry,
            ipc::commands::list_monitors,
            ipc::commands::get_now_playing,
            ipc::commands::list_players,
            ipc::commands::list_plugins,
            ipc::commands::check_plugin_zip,
            ipc::commands::install_staged_plugin,
            ipc::commands::discard_staged_plugin,
            ipc::commands::set_plugin_enabled,
            ipc::commands::remove_plugin,
            ipc::commands::track_latencies,
            ipc::commands::get_lyrics,
            ipc::commands::get_clock,
            ipc::commands::nudge_offset,
            ipc::commands::set_song_offset,
            ipc::commands::sync_lab,
            ipc::commands::start_calibration,
            ipc::commands::stop_calibration,
            ipc::commands::apply_calibration,
            ipc::commands::get_calibration,
            ipc::commands::clock_applied,
            ipc::commands::export_diagnostics,
            ipc::commands::list_audio_devices,
            ipc::commands::set_device_offset,
            ipc::commands::reload_lyrics,
            ipc::commands::get_overlay_layout,
            ipc::commands::report_overlay_size,
            ipc::commands::overlay_layout_ready,
            ipc::commands::overlay_rendered,
            ipc::commands::enter_edit_mode,
            ipc::commands::overlay_edit_update,
            ipc::commands::overlay_edit_monitor,
            ipc::commands::save_overlay_geometry,
            ipc::commands::cancel_edit_mode,
            ipc::commands::toggle_overlay,
            ipc::commands::set_overlay_preview,
            ipc::commands::list_fonts,
            ipc::commands::hotkey_status,
            ipc::commands::suspend_hotkeys,
            ipc::commands::overlay_diagnostics,
            ipc::commands::run_overlay_selftest,
            ipc::commands::app_info,
            ipc::commands::open_logs_folder,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            hotkeys::apply_settings(&handle);
            tray::create(&handle)?;
            overlay::setup(&handle)?;
            ipc::apply_autostart(&handle, handle.state::<settings::SettingsState>().get().behavior.autostart);
            // Session filter for the Windows media session (PLAN.md 5.8, 13.4): Spotify unless
            // the settings name another player.
            let player = handle.state::<settings::SettingsState>().get().player.app_id;
            app_state::start(handle.clone(), sources::smtc::SmtcSource::new(player).spawn());
            plugins::init(&handle);
            audio_devices::spawn_watcher(handle.clone());

            if let Some(mode) = selftest_mode.clone() {
                // Give the webview time to load and paint before measuring.
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(3000));
                    let report = overlay::selftest::run_and_record(&handle);
                    if mode == "exit" {
                        handle.exit(if report.failed == 0 { 0 } else { 1 });
                    }
                });
            } else if !autostarted && !handle.state::<settings::SettingsState>().get().behavior.start_minimized {
                settings_window::open(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Runs on the main thread: never lock overlay state here (see overlay/mod.rs).
            if window.label() == overlay::LABEL
                && let WindowEvent::ScaleFactorChanged { .. } = event
            {
                let app: AppHandle = window.app_handle().clone();
                std::thread::spawn(move || overlay::force_relayout(&app));
            }
            // The overlay has no frame and never the focus, so a close request comes from outside,
            // e.g. `taskkill` without /F. Closing only the overlay would leave the app running
            // without it; quit like the tray does instead.
            if window.label() == overlay::LABEL
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                tracing::info!("overlay close requested from outside, quitting");
                window.app_handle().exit(0);
            }
            // The appearance page may close with the window: end its preview.
            if window.label() == settings_window::LABEL && matches!(event, WindowEvent::Destroyed) {
                let app: AppHandle = window.app_handle().clone();
                std::thread::spawn(move || overlay::set_preview(&app, false));
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building LyricHub");

    app.run(|_app, event| {
        // Closing the settings window destroys it; the app lives on in the tray. Only an
        // explicit exit (tray "Beenden", code set) ends the process.
        match event {
            RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
            // Plugins get their shutdown; the job object ends them in any case (plugins/process.rs).
            RunEvent::Exit => plugins::shutdown_all(_app),
            _ => {}
        }
    });
}
