//! Plugins (PLAN-plugins.md): separate programs that LyricHub installs from a ZIP, starts,
//! watches and ends; they talk JSON-RPC over stdin/stdout. Version 1 of the protocol knows word
//! times only, and for now only LyricHub's own plugins exist (no documented API for others).
//!
//! Program files live in %APPDATA%/LyricHub/plugins/<id>, large data such as models in
//! %LOCALAPPDATA%/LyricHub/plugins/<id>. Which plugins are on is in the settings
//! (`plugins.enabled`); a newly installed plugin is off.

pub mod install;
pub mod manifest;
pub mod process;
pub mod supervisor;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::settings::SettingsState;
use manifest::{API_VERSION, Manifest};
use process::Event;
use supervisor::{Config, State, Supervisor};

/// Event with the plugin list, sent on every change.
pub const EVENT: &str = "plugins";

pub fn program_dir() -> PathBuf {
    crate::paths::data_dir().join("plugins")
}

pub fn data_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("LyricHub")
        .join("plugins")
}

/// A plugin as the settings page shows it.
#[derive(Serialize, Clone, Debug)]
pub struct PluginInfo {
    pub id: String,
    /// Folder name when the manifest is broken.
    pub name: String,
    pub version: String,
    pub description: BTreeMap<String, String>,
    pub permissions: Vec<String>,
    pub homepage: Option<String>,
    pub enabled: bool,
    /// "off", "starting", "running", "restarting", "incompatible", "failed" or "invalid".
    pub state: &'static str,
    /// Why it failed, or when it restarts.
    pub detail: Option<String>,
    /// The plugin's own status line and progress 0..1.
    pub status_text: Option<String>,
    pub progress: Option<f64>,
}

/// A checked ZIP, shown for confirmation before it is installed.
#[derive(Serialize, Clone, Debug)]
pub struct ZipInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: BTreeMap<String, String>,
    pub permissions: Vec<String>,
    pub sha256: String,
    pub bytes: usize,
    pub compatible: bool,
    /// Version that this ZIP replaces.
    pub installed_version: Option<String>,
}

struct Entry {
    manifest: Result<Manifest, String>,
    state: Option<State>,
    status_text: Option<String>,
    progress: Option<f64>,
    supervisor: Option<Supervisor>,
}

#[derive(Default)]
pub struct PluginsState {
    entries: Mutex<BTreeMap<String, Entry>>,
    /// The ZIP the user is asked to confirm.
    staged: Mutex<Option<Vec<u8>>>,
}

/// Reads the installed plugins and starts the ones that are on.
pub fn init(app: &AppHandle) {
    let found = install::scan(&program_dir());
    let enabled = app.state::<SettingsState>().get().plugins.enabled;
    {
        let state = app.state::<PluginsState>();
        let mut entries = state.entries.lock().unwrap();
        for (id, manifest) in found {
            entries.insert(id, Entry { manifest, state: None, status_text: None, progress: None, supervisor: None });
        }
        tracing::info!(installed = entries.len(), enabled = enabled.len(), "plugins found");
    }
    for id in enabled {
        start(app, &id);
    }
}

fn start(app: &AppHandle, id: &str) {
    let manifest = {
        let state = app.state::<PluginsState>();
        let entries = state.entries.lock().unwrap();
        match entries.get(id).map(|e| &e.manifest) {
            Some(Ok(m)) if m.compatible() && entries[id].supervisor.is_none() => m.clone(),
            _ => return,
        }
    };
    let dir = program_dir().join(id);
    let data = data_root().join(id);
    if let Err(e) = std::fs::create_dir_all(&data) {
        tracing::warn!(plugin = id, error = %e, "cannot create the plugin's data folder");
    }
    let language = match crate::i18n::current(app) {
        crate::i18n::Lang::De => "de",
        crate::i18n::Lang::En => "en",
    };
    let config = Config {
        exe: dir.join(&manifest.executable),
        dir: dir.clone(),
        init: json!({
            "api": API_VERSION,
            "lyrichub_version": env!("CARGO_PKG_VERSION"),
            "language": language,
            "program_dir": dir,
            "data_dir": data,
        }),
    };
    let on_state = {
        let (app, id) = (app.clone(), id.to_owned());
        Arc::new(move |s: State| {
            tracing::info!(plugin = %id, state = ?s, "plugin state");
            update(&app, &id, |e| e.state = Some(s.clone()));
        })
    };
    let on_event = {
        let (app, id) = (app.clone(), id.to_owned());
        Arc::new(move |e: Event| match e {
            Event::Status { text, progress } => update(&app, &id, |entry| {
                entry.status_text = Some(text.clone()).filter(|t| !t.is_empty());
                entry.progress = progress;
            }),
            Event::Log { level, text } => match level.as_str() {
                "error" | "warn" => tracing::warn!(plugin = %id, "{text}"),
                _ => tracing::info!(plugin = %id, "{text}"),
            },
            Event::Stderr(line) => tracing::debug!(plugin = %id, "stderr: {line}"),
            // The capabilities follow in P2 (word_times).
            Event::Notification { method, .. } => tracing::debug!(plugin = %id, method, "plugin notification"),
        })
    };
    let supervisor = Supervisor::start(config, on_state, on_event);
    if let Some(entry) = app.state::<PluginsState>().entries.lock().unwrap().get_mut(id) {
        entry.supervisor = Some(supervisor);
    }
}

/// Ends a plugin. The supervisor leaves the map first: its thread reports states, which need the lock.
fn stop(app: &AppHandle, id: &str) {
    let supervisor = app.state::<PluginsState>().entries.lock().unwrap().get_mut(id).and_then(|e| e.supervisor.take());
    if let Some(s) = supervisor {
        s.stop();
    }
    update(app, id, |e| {
        e.state = None;
        e.status_text = None;
        e.progress = None;
    });
}

fn update(app: &AppHandle, id: &str, change: impl FnOnce(&mut Entry)) {
    let changed = {
        let state = app.state::<PluginsState>();
        let mut entries = state.entries.lock().unwrap();
        entries.get_mut(id).map(change).is_some()
    };
    if changed && let Err(e) = app.emit(EVENT, list(app)) {
        tracing::warn!(error = %e, "could not emit plugins");
    }
}

pub fn list(app: &AppHandle) -> Vec<PluginInfo> {
    let enabled = app.state::<SettingsState>().get().plugins.enabled;
    let state = app.state::<PluginsState>();
    let entries = state.entries.lock().unwrap();
    entries
        .iter()
        .map(|(id, e)| {
            let on = enabled.contains(id);
            let (state, detail) = match (&e.manifest, &e.state) {
                (Err(why), _) => ("invalid", Some(why.clone())),
                (Ok(m), _) if !m.compatible() => {
                    ("incompatible", Some(format!("plugin speaks protocol {}, LyricHub {API_VERSION}", m.api)))
                }
                (Ok(_), None) => ("off", None),
                (Ok(_), Some(s)) => match s {
                    State::Starting => ("starting", None),
                    State::Running => ("running", None),
                    State::Restarting { attempt, in_s } => ("restarting", Some(format!("{attempt}/{in_s}"))),
                    State::Incompatible(why) => ("incompatible", Some(why.clone())),
                    State::Failed(why) => ("failed", Some(why.clone())),
                },
            };
            let m = e.manifest.as_ref().ok();
            PluginInfo {
                id: id.clone(),
                name: m.map(|m| m.name.clone()).unwrap_or_else(|| id.clone()),
                version: m.map(|m| m.version.clone()).unwrap_or_default(),
                description: m.map(|m| m.description.clone()).unwrap_or_default(),
                permissions: m.map(|m| m.permissions.clone()).unwrap_or_default(),
                homepage: m.and_then(|m| m.homepage.clone()),
                enabled: on,
                state,
                detail,
                status_text: e.status_text.clone(),
                progress: e.progress,
            }
        })
        .collect()
}

pub fn set_enabled(app: &AppHandle, id: &str, on: bool) -> Result<Vec<PluginInfo>, String> {
    if !app.state::<PluginsState>().entries.lock().unwrap().contains_key(id) {
        return Err(format!("plugin {id} is not installed"));
    }
    let (before, after) = app.state::<SettingsState>().update(|s| {
        s.plugins.enabled.retain(|x| x != id);
        if on {
            s.plugins.enabled.push(id.to_owned());
        }
    });
    crate::ipc::settings_changed(app, &before, &after);
    stop(app, id);
    if on {
        start(app, id);
    }
    Ok(list(app))
}

/// Checks a ZIP and keeps it for `install_staged`.
pub fn check_zip(app: &AppHandle, zip: Vec<u8>) -> Result<ZipInfo, String> {
    let checked = install::check(&zip)?;
    let m = checked.manifest;
    let installed_version = app
        .state::<PluginsState>()
        .entries
        .lock()
        .unwrap()
        .get(&m.id)
        .and_then(|e| e.manifest.as_ref().ok().map(|m| m.version.clone()));
    let info = ZipInfo {
        compatible: m.compatible(),
        id: m.id,
        name: m.name,
        version: m.version,
        description: m.description,
        permissions: m.permissions,
        sha256: checked.sha256,
        bytes: checked.bytes,
        installed_version,
    };
    tracing::info!(plugin = %info.id, version = %info.version, sha256 = %info.sha256, "plugin ZIP checked");
    *app.state::<PluginsState>().staged.lock().unwrap() = Some(zip);
    Ok(info)
}

/// Installs the checked ZIP. A plugin that was on is started again in its new version.
pub fn install_staged(app: &AppHandle) -> Result<Vec<PluginInfo>, String> {
    let zip = app.state::<PluginsState>().staged.lock().unwrap().take().ok_or("no plugin ZIP to install")?;
    let id = install::check(&zip)?.manifest.id;
    stop(app, &id);
    let manifest = install::install(&zip, &program_dir())?;
    tracing::info!(plugin = %id, version = %manifest.version, "plugin installed");
    app.state::<PluginsState>().entries.lock().unwrap().insert(
        id.clone(),
        Entry { manifest: Ok(manifest), state: None, status_text: None, progress: None, supervisor: None },
    );
    if app.state::<SettingsState>().get().plugins.enabled.contains(&id) {
        start(app, &id);
    }
    let _ = app.emit(EVENT, list(app));
    Ok(list(app))
}

pub fn discard_staged(app: &AppHandle) {
    app.state::<PluginsState>().staged.lock().unwrap().take();
}

pub fn uninstall(app: &AppHandle, id: &str) -> Result<Vec<PluginInfo>, String> {
    stop(app, id);
    install::remove(&program_dir(), &data_root(), id)?;
    app.state::<PluginsState>().entries.lock().unwrap().remove(id);
    let (before, after) = app.state::<SettingsState>().update(|s| s.plugins.enabled.retain(|x| x != id));
    crate::ipc::settings_changed(app, &before, &after);
    tracing::info!(plugin = id, "plugin removed");
    let _ = app.emit(EVENT, list(app));
    Ok(list(app))
}

/// Ends every plugin when LyricHub exits; they get their 2 s of grace in parallel.
pub fn shutdown_all(app: &AppHandle) {
    let supervisors: Vec<Supervisor> = {
        let state = app.state::<PluginsState>();
        let mut entries = state.entries.lock().unwrap();
        entries.values_mut().filter_map(|e| e.supervisor.take()).collect()
    };
    let threads: Vec<_> = supervisors.into_iter().map(|s| std::thread::spawn(move || s.stop())).collect();
    for t in threads {
        let _ = t.join();
    }
}
