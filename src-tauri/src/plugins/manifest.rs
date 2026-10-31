//! The plugin manifest `plugin.json` (PLAN-plugins.md 4.2). LyricHub reads it before it runs
//! anything: an invalid manifest keeps a plugin from being installed, an unknown API version
//! keeps it from being started.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Version of the plugin protocol this LyricHub speaks.
pub const API_VERSION: u32 = 1;
pub const MANIFEST_FILE: &str = "plugin.json";
/// What a plugin can do for LyricHub: word times for synced lyrics, lyrics for songs without
/// (PLAN-plugins-2.md 4.3).
const CAPABILITIES: [&str; 2] = ["word_timing", "lyrics_source"];
/// At most this many switches per plugin.
const MAX_SETTINGS: usize = 8;

/// A switch the plugin offers on LyricHub's plugin page (PLAN-plugins.md 4.7). Only `bool` for now.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PluginSetting {
    /// Lower case letters, digits and underscores.
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub default: bool,
    /// Label per language code ("de", "en").
    pub label: BTreeMap<String, String>,
    #[serde(default)]
    pub help: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Manifest {
    /// Folder name and settings key: lower case letters, digits and dashes.
    pub id: String,
    pub name: String,
    pub version: String,
    /// Protocol version the plugin speaks; only `API_VERSION` is started.
    pub api: u32,
    /// File name of the program inside the plugin folder.
    pub executable: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Shown before the plugin is switched on. Not enforced (no sandbox), except that only a
    /// plugin with `audio:player` learns the player's process ID.
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    /// Short description per language code ("de", "en").
    #[serde(default)]
    pub description: BTreeMap<String, String>,
    #[serde(default)]
    pub settings: Vec<PluginSetting>,
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Self, String> {
        let m: Manifest =
            serde_json::from_str(json.trim_start_matches('\u{feff}')).map_err(|e| format!("plugin.json: {e}"))?;
        m.validate()?;
        Ok(m)
    }

    pub fn compatible(&self) -> bool {
        self.api == API_VERSION
    }

    fn validate(&self) -> Result<(), String> {
        if !valid_id(&self.id) {
            return Err(format!("invalid plugin id {:?}", self.id));
        }
        if self.name.trim().is_empty() || self.name.chars().count() > 80 {
            return Err("plugin name missing or longer than 80 characters".into());
        }
        if self.version.trim().is_empty() || self.version.chars().count() > 32 {
            return Err("plugin version missing or longer than 32 characters".into());
        }
        let exe = &self.executable;
        if exe.contains(['/', '\\', ':']) || exe.starts_with('.') || !exe.to_lowercase().ends_with(".exe") {
            return Err(format!("executable must be a file name ending in .exe, not {exe:?}"));
        }
        if let Some(c) = self.capabilities.iter().find(|c| !CAPABILITIES.contains(&c.as_str())) {
            return Err(format!("unknown capability {c:?}"));
        }
        if let Some(p) = self.permissions.iter().find(|p| !valid_permission(p)) {
            return Err(format!("unknown permission {p:?}"));
        }
        if self.settings.len() > MAX_SETTINGS {
            return Err(format!("more than {MAX_SETTINGS} settings"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for s in &self.settings {
            let id_ok = (1..=32).contains(&s.id.len()) && s.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            if !id_ok || !seen.insert(s.id.as_str()) {
                return Err(format!("invalid or repeated setting id {:?}", s.id));
            }
            if s.kind != "bool" {
                return Err(format!("setting {:?}: only the type bool is known", s.id));
            }
            if s.label.values().all(|l| l.trim().is_empty()) || s.label.values().chain(s.help.values()).any(|l| l.chars().count() > 300) {
                return Err(format!("setting {:?}: label missing or text too long", s.id));
            }
        }
        Ok(())
    }
}

/// 1–64 lower case letters, digits and dashes, starting with a letter or digit.
pub fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && !id.starts_with('-')
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `audio:player`, or `network:<host>` with a plain host name.
fn valid_permission(p: &str) -> bool {
    if p == "audio:player" {
        return true;
    }
    p.strip_prefix("network:").is_some_and(|host| {
        !host.is_empty() && host.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{
        "id": "lyrichub-example", "name": "Example", "version": "0.1.0", "api": 1,
        "executable": "lyrichub-plugin-example.exe", "capabilities": ["word_timing"],
        "permissions": ["audio:player", "network:example.org"], "description": { "en": "Test" }
    }"#;

    fn with(field: &str, value: &str) -> String {
        let mut v: serde_json::Value = serde_json::from_str(GOOD).unwrap();
        v[field] = serde_json::from_str(value).unwrap();
        v.to_string()
    }

    #[test]
    fn a_complete_manifest_parses() {
        let m = Manifest::parse(GOOD).unwrap();
        assert_eq!((m.id.as_str(), m.api), ("lyrichub-example", 1));
        assert!(m.compatible() && m.permissions.contains(&"audio:player".to_owned()));
        // BOM written by some editors.
        assert!(Manifest::parse(&format!("\u{feff}{GOOD}")).is_ok());
    }

    #[test]
    fn unsafe_or_unknown_fields_are_rejected() {
        for (field, value) in [
            ("id", r#""Example""#),
            ("id", r#""../evil""#),
            ("id", r#""-x""#),
            ("executable", r#""..\\evil.exe""#),
            ("executable", r#""bin/plugin.exe""#),
            ("executable", r#""C:plugin.exe""#),
            ("executable", r#""plugin.bat""#),
            ("capabilities", r#"["draw_overlay"]"#),
            ("permissions", r#"["filesystem"]"#),
            ("permissions", r#"["network:https://x"]"#),
            ("name", r#""  ""#),
        ] {
            assert!(Manifest::parse(&with(field, value)).is_err(), "{field} = {value}");
        }
    }

    #[test]
    fn another_api_version_parses_but_is_not_compatible() {
        let m = Manifest::parse(&with("api", "2")).unwrap();
        assert!(!m.compatible());
    }

    #[test]
    fn settings_are_switches_with_a_label() {
        let with = |settings: &str| GOOD.replacen("\"description\"", &format!("\"settings\": {settings}, \"description\""), 1);
        let m = Manifest::parse(&with(r#"[{ "id": "auto_sync", "type": "bool", "default": true, "label": { "de": "Auto-Sync", "en": "Auto-Sync" } }]"#)).unwrap();
        assert_eq!((m.settings[0].id.as_str(), m.settings[0].default), ("auto_sync", true));
        assert!(Manifest::parse(&with(r#"[{ "id": "level", "type": "number", "label": { "en": "Level" } }]"#)).is_err(), "only bool");
        assert!(Manifest::parse(&with(r#"[{ "id": "Bad-Id", "type": "bool", "label": { "en": "x" } }]"#)).is_err());
        assert!(Manifest::parse(&with(r#"[{ "id": "x", "type": "bool", "label": {} }]"#)).is_err(), "label needed");
        assert!(Manifest::parse(&with(r#"[{ "id": "x", "type": "bool", "label": { "en": "a" } }, { "id": "x", "type": "bool", "label": { "en": "b" } }]"#)).is_err());
        let source = GOOD.replace(r#"["word_timing"]"#, r#"["lyrics_source"]"#);
        assert!(Manifest::parse(&source).is_ok(), "lyrics_source is a known capability");
    }
}
