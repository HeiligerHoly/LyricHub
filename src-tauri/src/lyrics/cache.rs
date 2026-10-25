//! Lyrics cache in SQLite, %APPDATA%/LyricHub/lyrichub.db (PLAN.md 7.3). Positive results stay;
//! misses are remembered for 7 days so LRCLIB is not asked again on every replay. The tables for
//! song and device offsets exist from the start and are filled from Phase 3 on.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

const SCHEMA_VERSION: i64 = 1;
pub const NEGATIVE_TTL_S: i64 = 7 * 24 * 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// `raw` holds LRC with timestamps.
    Lrc,
    /// `raw` holds plain lyrics without timing.
    Plain,
    /// Instrumental, `raw` is empty.
    None,
}

impl Format {
    fn as_str(self) -> &'static str {
        match self {
            Format::Lrc => "lrc",
            Format::Plain => "plain",
            Format::None => "none",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "lrc" => Format::Lrc,
            "plain" => Format::Plain,
            _ => Format::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cached {
    pub source: String,
    pub source_id: Option<i64>,
    pub format: Format,
    pub raw: String,
    pub instrumental: bool,
    pub fetched_at: i64,
}

#[derive(Debug, PartialEq)]
pub enum Lookup {
    Hit(Cached),
    /// LRCLIB had nothing recently.
    Negative,
    Miss,
}

pub struct Cache {
    conn: Connection,
}

impl Cache {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS tracks (
               key TEXT PRIMARY KEY, title TEXT NOT NULL, artist TEXT NOT NULL, album TEXT,
               duration_ms INTEGER, last_seen INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS lyrics (
               key TEXT PRIMARY KEY, source TEXT NOT NULL, source_id INTEGER, format TEXT NOT NULL,
               raw TEXT NOT NULL, synced INTEGER NOT NULL, instrumental INTEGER NOT NULL,
               fetched_at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS negative_cache (key TEXT PRIMARY KEY, until INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS song_overrides (
               key TEXT PRIMARY KEY, offset_ms INTEGER NOT NULL DEFAULT 0, user_lrc TEXT);
             CREATE TABLE IF NOT EXISTS device_offsets (
               device_id TEXT PRIMARY KEY, name TEXT, offset_ms INTEGER NOT NULL DEFAULT 0);",
        )?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < SCHEMA_VERSION {
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Self { conn })
    }

    pub fn touch_track(&self, key: &str, title: &str, artist: &str, album: Option<&str>, duration_ms: Option<u32>, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO tracks (key, title, artist, album, duration_ms, last_seen) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(key) DO UPDATE SET last_seen = excluded.last_seen",
            params![key, title, artist, album, duration_ms, now],
        )?;
        Ok(())
    }

    pub fn get(&self, key: &str, now: i64) -> rusqlite::Result<Lookup> {
        let hit = self
            .conn
            .query_row(
                "SELECT source, source_id, format, raw, instrumental, fetched_at FROM lyrics WHERE key = ?1",
                [key],
                |r| {
                    Ok(Cached {
                        source: r.get(0)?,
                        source_id: r.get(1)?,
                        format: Format::parse(&r.get::<_, String>(2)?),
                        raw: r.get(3)?,
                        instrumental: r.get::<_, i64>(4)? != 0,
                        fetched_at: r.get(5)?,
                    })
                },
            )
            .optional()?;
        if let Some(hit) = hit {
            return Ok(Lookup::Hit(hit));
        }
        let until: Option<i64> = self.conn.query_row("SELECT until FROM negative_cache WHERE key = ?1", [key], |r| r.get(0)).optional()?;
        Ok(match until {
            Some(until) if until > now => Lookup::Negative,
            _ => Lookup::Miss,
        })
    }

    pub fn put(&self, key: &str, c: &Cached) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO lyrics (key, source, source_id, format, raw, synced, instrumental, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![key, c.source, c.source_id, c.format.as_str(), c.raw, (c.format == Format::Lrc) as i64, c.instrumental as i64, c.fetched_at],
        )?;
        self.conn.execute("DELETE FROM negative_cache WHERE key = ?1", [key])?;
        Ok(())
    }

    pub fn put_negative(&self, key: &str, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO negative_cache (key, until) VALUES (?1, ?2)",
            params![key, now + NEGATIVE_TTL_S],
        )?;
        Ok(())
    }

    /// Drops what is known about a song, so the next lookup asks LRCLIB again.
    pub fn forget(&self, key: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM lyrics WHERE key = ?1", [key])?;
        self.conn.execute("DELETE FROM negative_cache WHERE key = ?1", [key])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached(format: Format) -> Cached {
        Cached { source: "lrclib".into(), source_id: Some(42), format, raw: "[00:01.00]a".into(), instrumental: false, fetched_at: 100 }
    }

    #[test]
    fn hit_miss_and_negative() {
        let c = Cache::in_memory().unwrap();
        assert_eq!(c.get("k", 0).unwrap(), Lookup::Miss);
        c.put_negative("k", 1000).unwrap();
        assert_eq!(c.get("k", 1000 + NEGATIVE_TTL_S - 1).unwrap(), Lookup::Negative);
        assert_eq!(c.get("k", 1000 + NEGATIVE_TTL_S).unwrap(), Lookup::Miss);
        c.put("k", &cached(Format::Lrc)).unwrap();
        assert_eq!(c.get("k", 0).unwrap(), Lookup::Hit(cached(Format::Lrc)));
    }

    #[test]
    fn forget_and_replace() {
        let c = Cache::in_memory().unwrap();
        c.put("k", &cached(Format::Plain)).unwrap();
        c.put("k", &cached(Format::Lrc)).unwrap();
        assert!(matches!(c.get("k", 0).unwrap(), Lookup::Hit(Cached { format: Format::Lrc, .. })));
        c.forget("k").unwrap();
        assert_eq!(c.get("k", 0).unwrap(), Lookup::Miss);
    }

    #[test]
    fn schema_version_and_tracks() {
        let c = Cache::in_memory().unwrap();
        let v: i64 = c.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        c.touch_track("k", "Title", "Artist", None, Some(1000), 5).unwrap();
        c.touch_track("k", "Title", "Artist", None, Some(1000), 9).unwrap();
        let seen: i64 = c.conn.query_row("SELECT last_seen FROM tracks WHERE key='k'", [], |r| r.get(0)).unwrap();
        assert_eq!(seen, 9);
    }
}
