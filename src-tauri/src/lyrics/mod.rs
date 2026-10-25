//! Lyrics pipeline (PLAN.md 3.3, 7): LRC parser, matcher, LRCLIB client, SQLite cache, the
//! worker that ties them together, and romaji for Japanese lines.

pub mod cache;
pub mod lrclib;
pub mod matcher;
pub mod parser;
pub mod romaji;
pub mod service;
pub mod types;
