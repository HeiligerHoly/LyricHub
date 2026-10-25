//! Minimal SMTC read for Phase 0: one snapshot of the Spotify media session via WinRT.
//! Proves the Rust path (windows crate, Windows.Media.Control) before Phase 1 builds the
//! event-driven source in `sources/smtc.rs`. Must run off the UI thread: `join()` blocks.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};

/// 100 ns ticks between 1601-01-01 (WinRT DateTime epoch) and 1970-01-01.
const EPOCH_DIFF_TICKS: i64 = 116_444_736_000_000_000;

#[derive(Serialize, Clone, Debug)]
pub struct NowPlaying {
    pub app_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub status: &'static str,
    /// Position as reported at `LastUpdatedTime`.
    pub position_ms: i64,
    /// Position extrapolated to now (PLAN.md 5.4); equals `position_ms` when not playing.
    pub position_now_ms: i64,
    pub end_ms: i64,
    /// How old Spotify's last timeline update is.
    pub last_update_age_ms: i64,
}

fn status_name(s: PlaybackStatus) -> &'static str {
    match s {
        PlaybackStatus::Playing => "Playing",
        PlaybackStatus::Paused => "Paused",
        PlaybackStatus::Stopped => "Stopped",
        PlaybackStatus::Changing => "Changing",
        PlaybackStatus::Opened => "Opened",
        PlaybackStatus::Closed => "Closed",
        _ => "Unknown",
    }
}

fn now_winrt_ticks() -> i64 {
    let since_unix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    EPOCH_DIFF_TICKS + (since_unix.as_nanos() / 100) as i64
}

/// Returns the first session whose AUMID contains `app_filter`, or None.
pub fn snapshot(app_filter: &str) -> windows::core::Result<Option<NowPlaying>> {
    let manager = SessionManager::RequestAsync()?.join()?;
    for session in manager.GetSessions()? {
        let app_id = session.SourceAppUserModelId()?.to_string();
        if !app_id.contains(app_filter) {
            continue;
        }
        let props = session.TryGetMediaPropertiesAsync()?.join()?;
        let timeline = session.GetTimelineProperties()?;
        let status = session.GetPlaybackInfo()?.PlaybackStatus()?;

        let position_ms = timeline.Position()?.Duration / 10_000;
        let age_ms = (now_winrt_ticks() - timeline.LastUpdatedTime()?.UniversalTime) / 10_000;
        let position_now_ms = if status == PlaybackStatus::Playing { position_ms + age_ms } else { position_ms };

        return Ok(Some(NowPlaying {
            app_id,
            title: props.Title()?.to_string(),
            artist: props.Artist()?.to_string(),
            album: props.AlbumTitle()?.to_string(),
            status: status_name(status),
            position_ms,
            position_now_ms,
            end_ms: timeline.EndTime()?.Duration / 10_000,
            last_update_age_ms: age_ms,
        }));
    }
    Ok(None)
}
