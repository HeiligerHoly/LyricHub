//! Per-user data locations. Everything user-specific lives under %APPDATA%/LyricHub (PLAN.md 14.2).

use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("LyricHub")
}
