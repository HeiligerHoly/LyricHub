//! Per-user data locations. Everything user-specific lives under %APPDATA%/LyricHub (PLAN.md 14.2).

use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("LyricHub")
}

/// WebView2's data folder, which Tauri puts under %LOCALAPPDATA%/<identifier>.
fn webview_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("dev.lyrichub.app").join("EBWebView"))
}

/// Components WebView2 downloaded before `--disable-component-update` (settings_window.rs):
/// Widevine DRM, update packages, filter lists, speech and hyphenation data, together about
/// 60 MB that LyricHub never uses. Best effort; folders in use stay until the next start.
pub fn drop_webview_components() {
    let Some(dir) = webview_dir() else { return };
    for name in ["WidevineCdm", "component_crx_cache", "Subresource Filter", "Speech Recognition", "hyphen-data"] {
        let path = dir.join(name);
        if path.exists() && std::fs::remove_dir_all(&path).is_ok() {
            tracing::info!(folder = name, "removed unused WebView2 component");
        }
    }
}
