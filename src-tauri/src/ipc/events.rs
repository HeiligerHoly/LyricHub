//! Event names, backend → frontend (PLAN.md 4.4). Payload types live next to their owners.

/// `TrackInfo | null`: song change, metadata update (cover loaded), player gone.
pub const TRACK: &str = "track";
/// No payload: the position jumped to the start of a song, a new title is probably on its
/// way (Spotify sends it up to 300 ms later). The overlay fades the old title out meanwhile.
pub const TRACK_CHANGING: &str = "track-changing";
/// `{ status }`: playing, paused, …
pub const PLAYBACK: &str = "playback";
/// `ClockEvent`: position anchor on status changes, corrections, seeks and every 2 s.
pub const CLOCK: &str = "clock";
/// `LyricsEvent`: lyrics or their state (loading, missing, …) for a track key.
pub const LYRICS: &str = "lyrics";
/// `{ offset_ms, delta_ms }`: a nudge hotkey changed the song offset; the overlay shows a toast.
pub const NUDGE: &str = "nudge";
/// `{ text }`: a short notice for the overlay, e.g. a new audio device.
pub const NOTICE: &str = "notice";
/// The complete `Settings` object after every change.
pub const SETTINGS: &str = "settings";
/// `LayoutEvent`: overlay content size, alignment and edit mode. Extends PLAN.md's
/// `overlay-mode { edit }`, which a single event replaces so mode and geometry never disagree.
pub const OVERLAY_LAYOUT: &str = "overlay-layout";
/// `CalibrationView`: state of the calibration assistant or the sync protocol.
pub const CALIBRATION: &str = "calibration";
