//! Installing plugins from a ZIP (PLAN-plugins.md 4.3). The archive is checked completely before
//! anything is written: a valid `plugin.json`, its program inside, no path that leaves the plugin
//! folder, no links, size limits. It is unpacked into a staging folder next to the target and
//! only then moved into place, so a failed install leaves the old version intact.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::manifest::{MANIFEST_FILE, Manifest, valid_id};

const MAX_ZIP_BYTES: usize = 256 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 2000;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// A checked archive: what the user confirms before it is installed.
#[derive(Debug)]
pub struct Checked {
    pub manifest: Manifest,
    pub sha256: String,
    pub bytes: usize,
    /// Folder inside the archive that holds `plugin.json`: "" or one top-level folder.
    prefix: PathBuf,
}

/// Checks an archive without writing anything.
pub fn check(zip: &[u8]) -> Result<Checked, String> {
    if zip.len() > MAX_ZIP_BYTES {
        return Err(format!("the archive is larger than {} MB", MAX_ZIP_BYTES / 1024 / 1024));
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).map_err(|e| format!("not a ZIP archive: {e}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(format!("the archive has more than {MAX_ENTRIES} entries"));
    }
    let mut paths = Vec::with_capacity(archive.len());
    let mut unpacked = 0u64;
    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i).map_err(|e| format!("damaged entry: {e}"))?;
        let name = entry.name().map(|n| n.into_owned()).unwrap_or_else(|_| "?".into());
        if entry.is_symlink() {
            return Err(format!("links are not allowed: {name}"));
        }
        // enclosed_name() would defuse these, but an archive that has them is not trusted.
        if name.starts_with(['/', '\\']) || name.chars().nth(1) == Some(':') {
            return Err(format!("absolute path in the archive: {name}"));
        }
        let path = entry.enclosed_name().ok_or_else(|| format!("unsafe path in the archive: {name}"))?;
        unpacked = unpacked.saturating_add(entry.size());
        if !entry.is_dir() {
            paths.push(path);
        }
    }
    if unpacked > MAX_UNPACKED_BYTES {
        return Err(format!("the archive unpacks to more than {} MB", MAX_UNPACKED_BYTES / 1024 / 1024));
    }

    // plugin.json at the root, or inside the one folder that holds everything.
    let prefix = if paths.iter().any(|p| p == Path::new(MANIFEST_FILE)) {
        PathBuf::new()
    } else {
        let tops: std::collections::BTreeSet<_> = paths.iter().filter_map(|p| p.components().next()).collect();
        match tops.into_iter().collect::<Vec<_>>().as_slice() {
            [only] if paths.iter().any(|p| p == &Path::new(only.as_os_str()).join(MANIFEST_FILE)) => {
                PathBuf::from(only.as_os_str())
            }
            _ => return Err(format!("{MANIFEST_FILE} is missing")),
        }
    };

    let mut manifest_json = String::new();
    archive
        .by_path(prefix.join(MANIFEST_FILE))
        .map_err(|e| format!("{MANIFEST_FILE}: {e}"))?
        .take(MAX_MANIFEST_BYTES)
        .read_to_string(&mut manifest_json)
        .map_err(|e| format!("{MANIFEST_FILE}: {e}"))?;
    let manifest = Manifest::parse(&manifest_json)?;
    if !paths.iter().any(|p| p == &prefix.join(&manifest.executable)) {
        return Err(format!("the program {} named in {MANIFEST_FILE} is missing", manifest.executable));
    }
    let sha256 = Sha256::digest(zip).iter().map(|b| format!("{b:02x}")).collect();
    Ok(Checked { manifest, sha256, bytes: zip.len(), prefix })
}

/// Unpacks a checked archive into `<plugins>/<id>`, replacing an older version. The plugin
/// must not be running.
pub fn install(zip: &[u8], plugins: &Path) -> Result<Manifest, String> {
    let checked = check(zip)?;
    let id = checked.manifest.id.clone();
    std::fs::create_dir_all(plugins).map_err(|e| format!("cannot create the plugin folder: {e}"))?;
    let staging = plugins.join(format!(".staging-{id}"));
    let _ = std::fs::remove_dir_all(&staging);
    let result = unpack(zip, &checked.prefix, &staging).and_then(|()| {
        let target = plugins.join(&id);
        if target.exists() {
            std::fs::remove_dir_all(&target).map_err(|e| format!("cannot replace the old version: {e}"))?;
        }
        std::fs::rename(&staging, &target).map_err(|e| format!("cannot move the plugin into place: {e}"))
    });
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result.map(|()| checked.manifest)
}

fn unpack(zip: &[u8], prefix: &Path, into: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        // check() made sure every path is enclosed and every file lies under the prefix.
        let Some(path) = entry.enclosed_name() else { continue };
        let Ok(rel) = path.strip_prefix(prefix) else { continue };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = into.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut file = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut file).map_err(|e| format!("{}: {e}", rel.display()))?;
    }
    Ok(())
}

/// Removes a plugin's program folder and its data folder.
pub fn remove(plugins: &Path, data: &Path, id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err(format!("invalid plugin id {id:?}"));
    }
    for dir in [plugins.join(id), data.join(id)] {
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| format!("cannot remove {}: {e}", dir.display()))?;
        }
    }
    Ok(())
}

/// Installed plugins: every folder with a manifest, valid or not.
pub fn scan(plugins: &Path) -> Vec<(String, Result<Manifest, String>)> {
    let Ok(dirs) = std::fs::read_dir(plugins) else { return Vec::new() };
    let mut out: Vec<_> = dirs
        .filter_map(Result::ok)
        .filter(|d| d.path().is_dir())
        .filter_map(|d| {
            let name = d.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            let manifest = std::fs::read_to_string(d.path().join(MANIFEST_FILE))
                .map_err(|e| format!("{MANIFEST_FILE}: {e}"))
                .and_then(|json| Manifest::parse(&json))
                .and_then(|m| if m.id == name { Ok(m) } else { Err(format!("folder {name:?} holds plugin {:?}", m.id)) });
            Some((name, manifest))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    const MANIFEST: &str = r#"{ "id": "demo", "name": "Demo", "version": "1.0.0", "api": 1, "executable": "demo.exe" }"#;

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, data) in files {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lyrichub-plugin-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_plugin_installs_from_the_root_or_one_folder_and_replaces_the_old_version() {
        let plugins = temp_dir("install");
        let zip = zip_of(&[("plugin.json", MANIFEST.as_bytes()), ("demo.exe", b"v1"), ("lib/a.dll", b"x")]);
        let checked = check(&zip).unwrap();
        assert_eq!(checked.manifest.id, "demo");
        assert_eq!(checked.sha256.len(), 64);
        install(&zip, &plugins).unwrap();
        assert_eq!(std::fs::read(plugins.join("demo/demo.exe")).unwrap(), b"v1");
        assert!(plugins.join("demo/lib/a.dll").exists());

        let nested = zip_of(&[("demo-1.1/plugin.json", MANIFEST.as_bytes()), ("demo-1.1/demo.exe", b"v2")]);
        install(&nested, &plugins).unwrap();
        assert_eq!(std::fs::read(plugins.join("demo/demo.exe")).unwrap(), b"v2");
        assert!(!plugins.join("demo/lib").exists(), "old files are gone");
        assert_eq!(scan(&plugins).len(), 1);

        remove(&plugins, &temp_dir("install-data"), "demo").unwrap();
        assert!(scan(&plugins).is_empty());
        let _ = std::fs::remove_dir_all(&plugins);
    }

    #[test]
    fn unsafe_or_incomplete_archives_are_rejected_before_writing() {
        let plugins = temp_dir("reject");
        for (what, zip) in [
            ("parent path", zip_of(&[("plugin.json", MANIFEST.as_bytes()), ("demo.exe", b""), ("../evil.txt", b"")])),
            ("absolute path", zip_of(&[("plugin.json", MANIFEST.as_bytes()), ("demo.exe", b""), ("/evil.txt", b"")])),
            ("no manifest", zip_of(&[("demo.exe", b"")])),
            ("no program", zip_of(&[("plugin.json", MANIFEST.as_bytes())])),
            ("two folders", zip_of(&[("a/plugin.json", MANIFEST.as_bytes()), ("a/demo.exe", b""), ("b/x", b"")])),
            ("bad manifest", zip_of(&[("plugin.json", b"{ \"id\": \"demo\" }"), ("demo.exe", b"")])),
            ("no zip", b"not a zip".to_vec()),
        ] {
            assert!(install(&zip, &plugins).is_err(), "{what}");
        }
        assert!(scan(&plugins).is_empty());
        assert!(!plugins.join("evil.txt").exists() && !plugins.parent().unwrap().join("evil.txt").exists());
        let _ = std::fs::remove_dir_all(&plugins);
    }

    #[test]
    fn a_folder_that_holds_another_plugin_is_reported() {
        let plugins = temp_dir("scan");
        std::fs::create_dir_all(plugins.join("other")).unwrap();
        std::fs::write(plugins.join("other/plugin.json"), MANIFEST).unwrap();
        let found = scan(&plugins);
        assert_eq!(found.len(), 1);
        assert!(found[0].1.is_err());
        let _ = std::fs::remove_dir_all(&plugins);
    }
}
