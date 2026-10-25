//! Settings: schema v1 and its JSON store, held in memory behind a mutex. Callers change
//! settings only through `SettingsState`, which saves every change; `crate::ipc` then emits
//! the `settings` event and applies side effects (overlay layout, autostart).

pub mod schema;
pub mod store;

use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;

pub use schema::*;

pub struct SettingsState {
    current: Mutex<Settings>,
    path: PathBuf,
}

impl SettingsState {
    pub fn load() -> Self {
        let path = store::default_path();
        let settings = store::load(&path);
        tracing::info!(path = %path.display(), "settings loaded");
        Self { current: Mutex::new(settings), path }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().unwrap().clone()
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Applies a JSON Merge Patch, validates and saves. Returns (before, after).
    pub fn patch(&self, patch: &Value) -> Result<(Settings, Settings), String> {
        let mut current = self.current.lock().unwrap();
        let before = current.clone();
        let mut value = serde_json::to_value(&*current).map_err(|e| e.to_string())?;
        store::merge_patch(&mut value, patch);
        let mut next: Settings = serde_json::from_value(value).map_err(|e| format!("invalid settings: {e}"))?;
        next.sanitize();
        self.commit(&mut current, next.clone());
        Ok((before, next))
    }

    /// Changes settings in code. Returns (before, after).
    pub fn update(&self, f: impl FnOnce(&mut Settings)) -> (Settings, Settings) {
        let mut current = self.current.lock().unwrap();
        let before = current.clone();
        let mut next = before.clone();
        f(&mut next);
        next.sanitize();
        self.commit(&mut current, next.clone());
        (before, next)
    }

    fn commit(&self, current: &mut Settings, next: Settings) {
        if *current == next {
            return;
        }
        if let Err(e) = store::save(&self.path, &next) {
            // Keep running with the new values; the next change tries to save again.
            tracing::error!(path = %self.path.display(), error = %e, "could not save settings");
        }
        *current = next;
    }
}
