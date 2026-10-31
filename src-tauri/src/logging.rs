//! Logging to stdout and to daily files in %APPDATA%/LyricHub/logs (PLAN.md 15: logs, not guesses).
//! Level via `LYRICHUB_LOG`, e.g. `LYRICHUB_LOG=debug`.

use std::path::PathBuf;

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn log_dir() -> PathBuf {
    crate::paths::data_dir().join("logs")
}

/// Self-test reports pile up during development, crash reports with a crash loop: keep the
/// newest few of each.
const REPORTS_KEPT: usize = 10;

/// Files in the log folder whose name starts with `prefix`, newest first.
pub fn newest_first(prefix: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(log_dir()) else { return Vec::new() };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    files.into_iter().map(|f| f.1).collect()
}

fn prune_reports() {
    for prefix in ["selftest-", "crash-"] {
        for path in newest_first(prefix).into_iter().skip(REPORTS_KEPT) {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// The last lines of the newest log file.
pub fn tail(lines: usize) -> Vec<String> {
    let newest = newest_first("lyrichub").into_iter().next();
    let Some(text) = newest.and_then(|p| std::fs::read_to_string(p).ok()) else { return Vec::new() };
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].iter().map(|l| (*l).to_owned()).collect()
}

pub fn init() {
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    // Release builds log info and up (about a quarter of the size); development builds also
    // the clock decisions. LYRICHUB_LOG overrides both.
    let default = if cfg!(debug_assertions) { "info,lyrichub_lib=debug" } else { "info" };
    let filter = EnvFilter::try_from_env("LYRICHUB_LOG").unwrap_or_else(|_| EnvFilter::new(default));
    prune_reports();
    let stdout = fmt::layer().with_writer(std::io::stdout);

    let file = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("lyrichub")
        .filename_suffix("log")
        .max_log_files(14)
        .build(&dir);

    match file {
        Ok(appender) => {
            // Blocking writer on purpose: few lines, and nothing is lost when the process exits.
            tracing_subscriber::registry()
                .with(filter)
                .with(stdout)
                .with(fmt::layer().with_ansi(false).with_writer(appender))
                .init();
            tracing::info!(dir = %dir.display(), "file logging enabled");
        }
        Err(e) => {
            tracing_subscriber::registry().with(filter).with(stdout).init();
            tracing::warn!(error = %e, dir = %dir.display(), "file logging unavailable, stdout only");
        }
    }
}
