//! Lyrics pipeline (PLAN.md 3.3, 7): LRC parser, matcher, LRCLIB client, SQLite cache and the
//! worker that ties them together.

pub mod lrclib;
pub mod matcher;
pub mod parser;
pub mod types;
