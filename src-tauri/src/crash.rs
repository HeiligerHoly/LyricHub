//! Crash traces (IDEEN.md, package A). The release build ends at once on a panic
//! (`panic = "abort"` in Cargo.toml); before that, this hook writes `logs/crash-<unix>.txt` with
//! version, thread, place and message, plus the last log lines with song details and the account
//! name hidden as in the diagnostics export. A native crash outside Rust's checks (an access violation in unsafe
//! code or inside WebView2) leaves no file here; Windows keeps it in its reliability history.
//! `LYRICHUB_CRASH_TEST=1` panics 3 s after the start, to check the trace.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

const PREFIX: &str = "crash-";
const LOG_LINES: usize = 40;

#[derive(Serialize, Debug)]
pub struct LastCrash {
    pub unix: u64,
    /// Thread, place and message: the second line of the report.
    pub summary: String,
    pub report: String,
}

pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current().name().unwrap_or("unnamed").to_owned();
        let place = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "an unknown place".into());
        let what = format!("panic in thread '{thread}' at {place}: {}", info.payload_as_str().unwrap_or("(no message)"));
        let what = crate::sync_lab::hide_user_folder(&what);
        // The file first: the logging itself may be what panicked.
        let path = write(&what);
        let report = path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
        tracing::error!(report = report.as_deref().unwrap_or("not written"), "{what}");
        default(info);
    }));
    if std::env::var_os("LYRICHUB_CRASH_TEST").is_some() {
        let _ = std::thread::Builder::new().name("crash-test".into()).spawn(|| {
            std::thread::sleep(Duration::from_secs(3));
            panic!("crash test (LYRICHUB_CRASH_TEST)");
        });
    }
}

fn write(what: &str) -> Option<PathBuf> {
    let unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let tail: Vec<String> = crate::logging::tail(LOG_LINES)
        .iter()
        .map(|l| crate::sync_lab::hide_user_folder(&crate::sync_lab::hide_song_in_line(l)))
        .collect();
    let path = crate::logging::log_dir().join(format!("{PREFIX}{unix}.txt"));
    std::fs::write(&path, report(what, unix, &tail)).ok()?;
    Some(path)
}

fn report(what: &str, unix: u64, tail: &[String]) -> String {
    format!(
        "LyricHub {} crashed at {} UTC\n{what}\n\nLast log lines (song details and account name hidden):\n{}\n",
        env!("CARGO_PKG_VERSION"),
        utc(unix),
        tail.join("\n")
    )
}

/// The newest crash report, for the diagnostics page and the export.
pub fn last() -> Option<LastCrash> {
    let path = crate::logging::newest_first(PREFIX).into_iter().next()?;
    let unix = path.file_stem()?.to_str()?.strip_prefix(PREFIX)?.parse().ok()?;
    let report = std::fs::read_to_string(&path).ok()?;
    let summary = report.lines().nth(1).unwrap_or_default().to_owned();
    Some(LastCrash { unix, summary, report })
}

/// "2026-10-31 14:21:30" for a Unix time (days to a civil date after Howard Hinnant).
fn utc(unix: u64) -> String {
    let (days, secs) = ((unix / 86_400) as i64, unix % 86_400);
    let z = days + 719_468;
    let (era, doe) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_times_read_as_utc_dates() {
        assert_eq!(utc(0), "1970-01-01 00:00:00");
        assert_eq!(utc(951_868_799), "2000-02-29 23:59:59");
        assert_eq!(utc(1_793_456_490), "2026-10-31 14:21:30");
    }

    #[test]
    fn the_report_names_the_panic_first() {
        let r = report("panic in thread 'lyrics' at src\\lyrics\\service.rs:12: boom", 1_793_456_490, &["a".into(), "b".into()]);
        let lines: Vec<&str> = r.lines().collect();
        assert_eq!(lines[0], format!("LyricHub {} crashed at 2026-10-31 14:21:30 UTC", env!("CARGO_PKG_VERSION")));
        assert_eq!(lines[1], "panic in thread 'lyrics' at src\\lyrics\\service.rs:12: boom");
        assert_eq!(&lines[lines.len() - 2..], ["a", "b"]);
    }
}
