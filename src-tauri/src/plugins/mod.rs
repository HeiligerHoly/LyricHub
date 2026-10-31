//! Plugins (PLAN-plugins.md): separate programs that LyricHub installs from a ZIP, starts,
//! watches and ends; they talk JSON-RPC over stdin/stdout. Version 1 of the protocol knows word
//! times only, and for now only LyricHub's own plugins exist (no documented API for others).
//!
//! A plugin with the capability `word_timing` hears `track`, `playback` (on every change and
//! every 2 s) and `lyrics` (synced lines with LyricHub's units), and answers with
//! `word_times`; those are checked, stored per lyrics fingerprint and replace the estimate
//! (words.rs, app_state.rs).
//!
//! Program files live in %APPDATA%/LyricHub/plugins/<id>, large data such as models in
//! %LOCALAPPDATA%/LyricHub/plugins/<id>. Which plugins are on is in the settings
//! (`plugins.enabled`); a newly installed plugin is off.

pub mod install;
pub mod manifest;
pub mod process;
pub mod supervisor;
pub mod words;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use crate::app_state::ClockEvent;
use crate::lyrics::types::{LyricsEvent, LyricsState};
use crate::settings::SettingsState;
use crate::sources::TrackInfo;
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
    /// The plugin's switches with their current values.
    pub settings: Vec<SettingInfo>,
}

/// One switch of a plugin, as the settings page shows it.
#[derive(Serialize, Clone, Debug)]
pub struct SettingInfo {
    pub id: String,
    pub label: BTreeMap<String, String>,
    pub help: BTreeMap<String, String>,
    pub value: bool,
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
            let running = s == State::Running;
            update(&app, &id, |e| e.state = Some(s.clone()));
            if running {
                send_current(&app, &id);
            }
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
            // LyricHub's own plugins write only panics there; they belong in the release log.
            Event::Stderr(line) => tracing::warn!(plugin = %id, "stderr: {line}"),
            Event::Notification { method, params } if method == "word_times" => on_word_times(&app, &id, &params),
            Event::Notification { method, params } if method == "lyrics_found" => on_lyrics_found(&app, &id, &params),
            Event::Notification { method, .. } => tracing::debug!(plugin = %id, method, "unknown plugin notification"),
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
    let values = app.state::<SettingsState>().get().plugins.values;
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
                settings: m
                    .map(|m| {
                        let stored = values.get(&m.id);
                        m.settings
                            .iter()
                            .map(|s| SettingInfo {
                                id: s.id.clone(),
                                label: s.label.clone(),
                                help: s.help.clone(),
                                value: stored.and_then(|v| v.get(&s.id)).copied().unwrap_or(s.default),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect()
}

/// Current values of a plugin's switches: the stored one, else the manifest's default.
fn setting_values(app: &AppHandle, m: &Manifest) -> BTreeMap<String, bool> {
    let stored = app.state::<SettingsState>().get().plugins.values.get(&m.id).cloned().unwrap_or_default();
    m.settings.iter().map(|s| (s.id.clone(), stored.get(&s.id).copied().unwrap_or(s.default))).collect()
}

/// A switch of a plugin that is installed and on; false otherwise.
pub fn setting_on(app: &AppHandle, plugin: &str, key: &str) -> bool {
    if !app.state::<SettingsState>().get().plugins.enabled.iter().any(|x| x == plugin) {
        return false;
    }
    let manifest = app.state::<PluginsState>().entries.lock().unwrap().get(plugin).and_then(|e| e.manifest.clone().ok());
    manifest.is_some_and(|m| setting_values(app, &m).get(key).copied().unwrap_or(false))
}

/// Changes a plugin's switch, tells the plugin and shows the lyrics again (Auto-Sync works on
/// them).
pub fn set_setting(app: &AppHandle, id: &str, key: &str, value: bool) -> Result<Vec<PluginInfo>, String> {
    let manifest = app.state::<PluginsState>().entries.lock().unwrap().get(id).and_then(|e| e.manifest.clone().ok());
    let Some(m) = manifest.filter(|m| m.settings.iter().any(|s| s.id == key)) else {
        return Err(format!("plugin {id} has no setting {key}"));
    };
    let (before, after) = app.state::<SettingsState>().update(|s| {
        s.plugins.values.entry(id.to_owned()).or_default().insert(key.to_owned(), value);
    });
    crate::ipc::settings_changed(app, &before, &after);
    if let Some((_, p)) = running(app, id) {
        p.notify("settings", json!(setting_values(app, &m)));
    }
    crate::app_state::refresh_lyrics(app);
    let list = list(app);
    let _ = app.emit(EVENT, &list);
    Ok(list)
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
    crate::app_state::forget_plugin_results(app, id);
    app.state::<PluginsState>().entries.lock().unwrap().remove(id);
    let (before, after) = app.state::<SettingsState>().update(|s| s.plugins.enabled.retain(|x| x != id));
    crate::ipc::settings_changed(app, &before, &after);
    tracing::info!(plugin = id, "plugin removed");
    let _ = app.emit(EVENT, list(app));
    Ok(list(app))
}

/// Running plugins with the capability `word_timing`, with their manifests.
/// Running plugins with any of these capabilities.
fn with_capability(app: &AppHandle, any_of: &[&str]) -> Vec<(Manifest, Arc<process::Process>)> {
    let state = app.state::<PluginsState>();
    let entries = state.entries.lock().unwrap();
    entries
        .values()
        .filter(|e| e.state == Some(State::Running))
        .filter_map(|e| {
            let m = e.manifest.as_ref().ok()?;
            let p = e.supervisor.as_ref()?.process()?;
            m.capabilities.iter().any(|c| any_of.contains(&c.as_str())).then(|| (m.clone(), p))
        })
        .collect()
}

fn word_timing(app: &AppHandle) -> Vec<(Manifest, Arc<process::Process>)> {
    with_capability(app, &["word_timing"])
}

/// Plugins that listen to the player: they need the track, the clock and the screen.
fn audio(app: &AppHandle) -> Vec<(Manifest, Arc<process::Process>)> {
    with_capability(app, &["word_timing", "lyrics_source"])
}

fn running(app: &AppHandle, id: &str) -> Option<(Manifest, Arc<process::Process>)> {
    let state = app.state::<PluginsState>();
    let entries = state.entries.lock().unwrap();
    let e = entries.get(id).filter(|e| e.state == Some(State::Running))?;
    Some((e.manifest.as_ref().ok()?.clone(), e.supervisor.as_ref()?.process()?))
}

fn track_params(t: Option<&TrackInfo>, manifest: &Manifest, app: &AppHandle) -> Value {
    let Some(t) = t else { return Value::Null };
    let app_id = app.state::<SettingsState>().get().player.app_id;
    // Only a plugin that says it captures the player's sound learns which process that is.
    let pid = manifest
        .permissions
        .iter()
        .any(|p| p == "audio:player")
        .then(|| crate::sources::smtc::player_pid(&app_id))
        .flatten();
    json!({
        "key": t.key, "title": t.title, "artist": t.artist, "album": t.album,
        "duration_ms": t.duration_ms, "player": { "app_id": app_id, "pid": pid },
    })
}

pub fn on_track(app: &AppHandle, track: Option<&TrackInfo>) {
    for (m, p) in audio(app) {
        p.notify("track", track_params(track, &m, app));
    }
}

/// Position valid now, with the moment as QueryPerformanceCounter time in 100 ns units: the unit
/// WASAPI uses for the capture position (PLAN-plugins.md 5.1). The overlay shows lines by
/// `position_ms - offset_ms`; word times belong on that clock.
fn playback_params(clock: &ClockEvent) -> Value {
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    let (mut count, mut freq) = (0i64, 0i64);
    // SAFETY: plain counter reads.
    unsafe {
        let _ = QueryPerformanceCounter(&mut count);
        let _ = QueryPerformanceFrequency(&mut freq);
    }
    let qpc_100ns = if freq > 0 { (count as i128 * 10_000_000 / freq as i128) as i64 } else { 0 };
    json!({
        "playing": clock.playing, "position_ms": clock.pos_ms, "rate": clock.rate, "qpc_100ns": qpc_100ns,
        "offset_ms": clock.offset_ms,
    })
}

pub fn on_playback(app: &AppHandle, clock: &ClockEvent) {
    let plugins = audio(app);
    if plugins.is_empty() {
        return;
    }
    let params = playback_params(clock);
    for (_, p) in plugins {
        p.notify("playback", params.clone());
    }
}

fn lyrics_params(event: &LyricsEvent) -> Option<Value> {
    if event.state != LyricsState::Synced {
        return None;
    }
    let lines: Vec<Value> = event
        .lines
        .iter()
        .map(|l| {
            json!({ "start_ms": l.start_ms, "end_ms": l.end_ms, "text": l.text, "romaji": l.romaji, "units": words::units(l) })
        })
        .collect();
    Some(json!({
        "key": event.key, "lyrics_hash": words::lyrics_hash(&event.lines), "lines": lines,
        "word_source": event.word_source,
    }))
}

pub fn on_lyrics(app: &AppHandle, event: &LyricsEvent) {
    let Some(params) = lyrics_params(event) else { return };
    for (_, p) in word_timing(app) {
        p.notify("lyrics", params.clone());
    }
}

/// A fullscreen app came to the front or left it (overlay/guard.rs): plugins that compute in the
/// background pause meanwhile (PLAN-plugins.md 5.4).
pub fn on_screen(app: &AppHandle, fullscreen: bool) {
    for (_, p) in audio(app) {
        p.notify("screen", json!({ "fullscreen": fullscreen }));
    }
}

fn fullscreen_now() -> bool {
    crate::overlay::guard::screen() != crate::overlay::guard::Screen::Normal
}

/// Asks the plugins with `lyrics_source` for lyrics of a song that has none, or only lyrics
/// without timestamps (PLAN-plugins-2.md 4.3).
fn lyrics_wanted_params(app: &AppHandle, event: &LyricsEvent) -> Option<Value> {
    if !matches!(event.state, LyricsState::Missing | LyricsState::Unsynced) {
        return None;
    }
    let now = app.state::<crate::app_state::NowPlayingState>().snapshot();
    let t = now.track.as_ref().filter(|t| t.key == event.key && !crate::app_state::plugin_lyrics_rejected(app, &t.key))?;
    let plain = (event.state == LyricsState::Unsynced).then(|| crate::app_state::plain_lyrics(app, &t.key)).flatten();
    Some(json!({
        "key": t.key, "title": t.title, "artist": t.artist, "album": t.album, "duration_ms": t.duration_ms, "plain": plain,
    }))
}

pub fn on_lyrics_wanted(app: &AppHandle, event: &LyricsEvent) {
    let Some(params) = lyrics_wanted_params(app, event) else { return };
    for (_, p) in with_capability(app, &["lyrics_source"]) {
        p.notify("lyrics_wanted", params.clone());
    }
}

/// A plugin that just started gets its settings and hears what plays now.
fn send_current(app: &AppHandle, id: &str) {
    let Some((m, p)) = running(app, id) else { return };
    if !m.settings.is_empty() {
        p.notify("settings", json!(setting_values(app, &m)));
    }
    let has = |c: &str| m.capabilities.iter().any(|x| x == c);
    if !has("word_timing") && !has("lyrics_source") {
        return;
    }
    let state = app.state::<crate::app_state::NowPlayingState>();
    let now = state.snapshot();
    p.notify("track", track_params(now.track.as_ref(), &m, app));
    let lyrics = state.lyrics();
    if has("word_timing")
        && let Some(params) = lyrics.as_ref().and_then(lyrics_params)
    {
        p.notify("lyrics", params);
    }
    if has("lyrics_source")
        && let Some(params) = lyrics.as_ref().and_then(|l| lyrics_wanted_params(app, l))
    {
        p.notify("lyrics_wanted", params);
    }
    p.notify("playback", playback_params(&crate::app_state::clock_event(app, "query")));
    p.notify("screen", json!({ "fullscreen": fullscreen_now() }));
}

fn on_lyrics_found(app: &AppHandle, id: &str, params: &Value) {
    let found = match words::parse_lyrics_found(params) {
        Ok(f) => f,
        Err(e) => return tracing::warn!(plugin = id, error = %e, "lyrics not readable"),
    };
    // Only a plugin that declared the capability may deliver lyrics.
    let version = {
        let state = app.state::<PluginsState>();
        let entries = state.entries.lock().unwrap();
        entries.get(id).and_then(|e| e.manifest.as_ref().ok()).filter(|m| m.capabilities.iter().any(|c| c == "lyrics_source")).map(|m| m.version.clone())
    };
    let Some(version) = version else {
        return tracing::warn!(plugin = id, "lyrics from a plugin without the capability lyrics_source ignored");
    };
    crate::app_state::store_plugin_lyrics(app, (id, &version), found);
}

fn on_word_times(app: &AppHandle, id: &str, params: &Value) {
    let (key, hash, answer) = match words::parse_word_times(params) {
        Ok(x) => x,
        Err(e) => return tracing::warn!(plugin = id, error = %e, "word times not readable"),
    };
    let Some(lines) = crate::app_state::lyrics_lines_for(app, &key) else {
        return tracing::info!(plugin = id, key = %key, "word times for lyrics LyricHub does not have");
    };
    if words::lyrics_hash(&lines) != hash {
        return tracing::info!(plugin = id, key = %key, "word times for other lyrics (fingerprint differs)");
    }
    let valid = words::valid_lines(&lines, &answer);
    tracing::info!(plugin = id, key = %key, lines = answer.len(), valid = valid.len(), "word times received");
    if valid.is_empty() {
        return;
    }
    let version = {
        let state = app.state::<PluginsState>();
        let entries = state.entries.lock().unwrap();
        entries.get(id).and_then(|e| e.manifest.as_ref().ok().map(|m| m.version.clone())).unwrap_or_default()
    };
    let json = serde_json::to_string(&valid).unwrap_or_default();
    crate::app_state::store_plugin_words(app, &key, &hash, (id, &version), &json);
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
