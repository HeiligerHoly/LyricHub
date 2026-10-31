//! Local LRC files (IDEEN.md, package B): `.lrc` files in %APPDATA%/LyricHub/lrc win over the
//! cache and LRCLIB. A file fits a song when its name reads "Artist - Title.lrc", or its `[ar:]`
//! and `[ti:]` tags name the song; the names are compared like LRCLIB candidates, the title more
//! strictly, as no song length backs the comparison up. The folder is
//! read again at every song change, so a new or edited file shows at the next one. Files never
//! leave the PC.

use std::io::Read;
use std::path::{Path, PathBuf};

use super::cache::{Cached, Format};
use super::{matcher, parser};

/// Enough of a file for its tags.
const HEAD_BYTES: u64 = 4096;
/// "Paper Planes" and "Paper Boats" pass LRCLIB's title threshold, where the length must match
/// too; here they must not.
const MIN_TITLE_SIMILARITY: f64 = 0.95;

pub fn dir() -> PathBuf {
    crate::paths::data_dir().join("lrc")
}

/// Name of a new file for a song: "Artist - Title.lrc", without characters Windows forbids.
pub fn file_name(title: &str, artist: &str) -> String {
    let clean = |s: &str| {
        let kept: String = s.chars().filter(|c| !c.is_control() && !r#"<>:"/\|?*"#.contains(*c)).collect();
        kept.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches('.').trim().to_owned()
    };
    format!("{} - {}.lrc", clean(artist), clean(title))
}

/// The best fitting file for a song and its text, if any.
pub fn find(dir: &Path, title: &str, artist: &str) -> Option<(PathBuf, String)> {
    let mut best: Option<(f64, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|x| x.eq_ignore_ascii_case("lrc")) {
            continue;
        }
        let by_name = path.file_stem().and_then(|s| s.to_str()).and_then(|stem| {
            let (file_artist, file_title) = stem.split_once(" - ")?;
            fits(file_title.trim(), file_artist.trim(), title, artist)
        });
        let Some(score) = by_name.or_else(|| by_tags(&path, title, artist)) else { continue };
        if best.as_ref().is_none_or(|b| score > b.0) {
            best = Some((score, path));
        }
    }
    let path = best?.1;
    let bytes = std::fs::read(&path).ok()?;
    Some((path, String::from_utf8_lossy(&bytes).into_owned()))
}

/// A local file as a cache entry, for `service::to_event`; it never goes into the cache.
pub fn cached(text: String, now: i64) -> Cached {
    let format = if parser::parse(&text).synced { Format::Lrc } else { Format::Plain };
    Cached { source: "local".into(), source_id: None, format, raw: text, instrumental: false, fetched_at: now }
}

fn by_tags(path: &Path, title: &str, artist: &str) -> Option<f64> {
    let mut head = Vec::new();
    std::fs::File::open(path).ok()?.take(HEAD_BYTES).read_to_end(&mut head).ok()?;
    let tags = parser::parse(&String::from_utf8_lossy(&head));
    fits(tags.title.as_deref()?, tags.artist.as_deref()?, title, artist)
}

fn fits(file_title: &str, file_artist: &str, title: &str, artist: &str) -> Option<f64> {
    let t = matcher::title_similarity(file_title, title);
    let a = matcher::artist_similarity(file_artist, artist);
    (t >= MIN_TITLE_SIMILARITY && a >= matcher::MIN_ARTIST_SIMILARITY).then_some(t * 0.6 + a * 0.4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similar_titles_are_not_the_same_song() {
        let alike = matcher::title_similarity("Paper Planes", "Paper Boats");
        assert!((matcher::MIN_TITLE_SIMILARITY..MIN_TITLE_SIMILARITY).contains(&alike), "{alike}");
    }

    #[test]
    fn new_files_get_a_name_windows_accepts() {
        assert_eq!(file_name("Paper Boats: Part 2?", "The Inventions / Guest"), "The Inventions Guest - Paper Boats Part 2.lrc");
        assert_eq!(file_name("Who?", "A.B."), "A.B - Who.lrc");
    }

    #[test]
    fn a_file_fits_by_name_or_by_tags() {
        let dir = std::env::temp_dir().join(format!("lyrichub-local-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Invented lyrics.
        std::fs::write(dir.join("The Inventions - Paper Boats.lrc"), "[00:01.00]paper boats\n").unwrap();
        std::fs::write(dir.join("track07.LRC"), "[ar:The Inventions]\n[ti:Stone Bridges]\n[00:02.00]stone bridges\n").unwrap();
        std::fs::write(dir.join("The Inventions - Paper Planes.txt"), "[00:03.00]not an lrc file\n").unwrap();
        assert_eq!(find(&dir, "Paper Boats", "The Inventions, Guest").map(|f| f.1).as_deref(), Some("[00:01.00]paper boats\n"));
        let (path, text) = find(&dir, "Stone Bridges", "The Inventions").unwrap();
        assert!(path.ends_with("track07.LRC") && text.contains("stone bridges"));
        assert_eq!(find(&dir, "Paper Planes", "The Inventions"), None);
        assert_eq!(find(&dir, "Paper Boats", "Someone Else"), None);
        assert_eq!(find(&dir.join("missing"), "Paper Boats", "The Inventions"), None);
        assert_eq!(cached("[00:01.00]a".into(), 0).format, Format::Lrc);
        assert_eq!(cached("just text".into(), 0).format, Format::Plain);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
