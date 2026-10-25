//! JSON store for the settings in %APPDATA%/LyricHub/settings.json (PLAN.md 4.2).
//!
//! Writes go to a temporary file that then replaces the real one, so a crash mid-write never
//! leaves a half-written file. A file that cannot be parsed is kept as
//! `settings.broken-<unix>.json` and replaced by defaults instead of blocking the start.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

use super::schema::{SCHEMA_VERSION, Settings};

pub fn default_path() -> PathBuf {
    crate::paths::data_dir().join("settings.json")
}

pub fn load(path: &Path) -> Settings {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            tracing::info!(path = %path.display(), "no settings file yet, using defaults");
            return Settings::default();
        }
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "settings unreadable, using defaults");
            return Settings::default();
        }
    };
    match parse(&text) {
        Ok(s) => s,
        Err(e) => {
            let unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let backup = path.with_file_name(format!("settings.broken-{unix}.json"));
            let kept = std::fs::rename(path, &backup).is_ok();
            tracing::warn!(error = %e, backup = %backup.display(), kept, "settings file broken, using defaults");
            Settings::default()
        }
    }
}

pub fn parse(text: &str) -> Result<Settings, serde_json::Error> {
    let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    let mut settings: Settings = serde_json::from_value(migrate(value))?;
    settings.sanitize();
    Ok(settings)
}

pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    // Replaces an existing file on Windows (MoveFileEx with MOVEFILE_REPLACE_EXISTING).
    std::fs::rename(&tmp, path)
}

/// Brings older files up to the current schema. v1 is the first version; a file without
/// `schema_version` predates it and is read as v1.
fn migrate(mut value: Value) -> Value {
    let version = value.get("schema_version").and_then(Value::as_u64).unwrap_or(0);
    if version > SCHEMA_VERSION as u64 {
        tracing::warn!(version, supported = SCHEMA_VERSION, "settings come from a newer LyricHub, unknown fields are dropped");
    }
    if let Value::Object(map) = &mut value {
        map.insert("schema_version".into(), Value::from(SCHEMA_VERSION));
    }
    value
}

/// JSON Merge Patch (RFC 7386): objects merge, `null` removes a key (its default applies),
/// everything else replaces.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(patch) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let Value::Object(map) = target else { unreachable!() };
    for (key, value) in patch {
        if value.is_null() {
            map.remove(key);
        } else {
            merge_patch(map.entry(key.clone()).or_insert(Value::Null), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::schema::Anchor;
    use serde_json::json;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lyrichub-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("settings.json")
    }

    #[test]
    fn save_and_load_roundtrip() {
        let path = temp_path("roundtrip");
        let mut s = Settings::default();
        s.overlay.anchor = Anchor::BottomRight;
        s.overlay.scale = 1.5;
        s.overlay.monitor_id = Some(r"\\.\DISPLAY2".into());
        save(&path, &s).unwrap();
        assert_eq!(load(&path), s);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn missing_file_gives_defaults() {
        let path = temp_path("missing");
        assert_eq!(load(&path), Settings::default());
    }

    #[test]
    fn broken_file_is_backed_up() {
        let path = temp_path("broken");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), Settings::default());
        assert!(!path.exists());
        let backups = std::fs::read_dir(path.parent().unwrap()).unwrap().count();
        assert_eq!(backups, 1);
    }

    #[test]
    fn file_without_version_and_partial_fields_loads() {
        let s = parse(r#"{ "overlay": { "anchor": "bl", "scale": 7 } }"#).unwrap();
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        assert_eq!(s.overlay.anchor, Anchor::BottomLeft);
        assert_eq!(s.overlay.scale, 3.0);
        assert_eq!(s.overlay.width_pct, 35.0);
        // BOM written by some editors
        assert!(parse("\u{feff}{}").is_ok());
    }

    #[test]
    fn merge_patch_follows_rfc7386() {
        let mut v = json!({ "overlay": { "anchor": "tc", "monitor_id": "X", "scale": 1.0 }, "behavior": { "autostart": false } });
        merge_patch(&mut v, &json!({ "overlay": { "scale": 2.0, "monitor_id": null }, "behavior": { "autostart": true } }));
        assert_eq!(v, json!({ "overlay": { "anchor": "tc", "scale": 2.0 }, "behavior": { "autostart": true } }));
    }
}
