//! Event names, backend → frontend (PLAN.md 4.4). Payload types live next to their owners.

/// `TrackInfo | null`: song change, metadata update (cover loaded), player gone.
pub const TRACK: &str = "track";
/// No payload: the position jumped to the start of a song, a new title is probably on its
/// way (Spotify sends it up to 300 ms later). The overlay fades the old title out meanwhile.
pub const TRACK_CHANGING: &str = "track-changing";
/// `{ status }`: playing, paused, … Phase 2 adds the `clock` anchor next to it.
pub const PLAYBACK: &str = "playback";
/// The complete `Settings` object after every change.
pub const SETTINGS: &str = "settings";
/// `LayoutEvent`: overlay content size, alignment and edit mode. Extends PLAN.md's
/// `overlay-mode { edit }`, which a single event replaces so mode and geometry never disagree.
pub const OVERLAY_LAYOUT: &str = "overlay-layout";
