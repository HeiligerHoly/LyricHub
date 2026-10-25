//! Playback sources (PLAN.md 4.1, 4.2). Phase 1 has one: the Windows media session (SMTC).

pub mod smtc;
pub mod types;

use std::sync::mpsc::Receiver;

pub use types::*;

pub trait PlaybackSource: Send + 'static {
    /// Starts the source on its own thread and returns its event stream.
    fn spawn(self) -> Receiver<SourceEvent>;
}
