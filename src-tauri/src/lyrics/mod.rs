//! Lyrics pipeline (PLAN.md 3.3, 7): LRC parser, matcher, LRCLIB client, SQLite cache, local
//! LRC files, the worker that ties them together, romaji for Japanese lines and estimated word
//! times.

pub mod cache;
pub mod local;
pub mod lrclib;
pub mod matcher;
pub mod parser;
pub mod romaji;
pub mod service;
pub mod types;
pub mod words;
