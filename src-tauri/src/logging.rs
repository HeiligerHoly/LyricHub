//! Logging to stdout and to daily files in %APPDATA%/LyricHub/logs (PLAN.md 15: logs, not guesses).
//! Level via `LYRICHUB_LOG`, e.g. `LYRICHUB_LOG=debug`.

use std::path::PathBuf;

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn log_dir() -> PathBuf {
    crate::paths::data_dir().join("logs")
}

pub fn init() {
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let filter = EnvFilter::try_from_env("LYRICHUB_LOG").unwrap_or_else(|_| EnvFilter::new("info,lyrichub_lib=debug"));
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
