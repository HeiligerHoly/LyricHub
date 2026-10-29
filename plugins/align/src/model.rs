//! The model files (PLAN-plugins.md 5.3): w2v-bert-2.0 music-lyrics CTC as int8 ONNX (MIT),
//! downloaded once from huggingface.co at a pinned revision. Interrupted downloads resume, and a
//! file only counts once its SHA-256 matches.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

const REPO: &str = "https://huggingface.co/redbaty/ytm-lyrics-aligner/resolve";
pub const REVISION: &str = "5a75dac46091289a9040ac88fca5cf288ea85699";

pub struct ModelFile {
    pub name: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

/// From the repository's SHA256SUMS at `REVISION`.
pub const FILES: [ModelFile; 2] = [
    ModelFile { name: "vocab.json", size: 11_566, sha256: "d5cfe4824d661f406b027e7ffcaa0847de938a0590fb78f148862c1d53309b27" },
    ModelFile { name: "aligner-int8.onnx", size: 587_455_736, sha256: "5bb8ae0e6ce301e366514c2210912d5454fd4e8433c76084d3932a1f1e481c20" },
];

pub fn total_size() -> u64 {
    FILES.iter().map(|f| f.size).sum()
}

/// Where the model lives inside the plugin's data folder.
pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("model")
}

pub fn onnx(dir: &Path) -> PathBuf {
    dir.join(FILES[1].name)
}

pub fn vocab(dir: &Path) -> PathBuf {
    dir.join(FILES[0].name)
}

/// All files are there. Their hashes were checked when they arrived.
pub fn ready(dir: &Path) -> bool {
    FILES.iter().all(|f| fs::metadata(dir.join(f.name)).is_ok_and(|m| m.len() == f.size))
}

/// Downloads what is missing. `progress` gets the bytes done and the total.
pub fn download(dir: &Path, mut progress: impl FnMut(u64, u64)) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("model folder: {e}"))?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build())
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .user_agent(concat!("lyrichub-align/", env!("CARGO_PKG_VERSION"), " (+https://github.com/HeiligerHoly/LyricHub)"))
        .build()
        .into();
    let total = total_size();
    let mut before = 0;
    for f in &FILES {
        let target = dir.join(f.name);
        if fs::metadata(&target).is_ok_and(|m| m.len() == f.size) {
            before += f.size;
            continue;
        }
        fetch(&agent, f, &target, |done| progress(before + done, total))?;
        before += f.size;
    }
    Ok(())
}

fn fetch(agent: &ureq::Agent, f: &ModelFile, target: &Path, mut progress: impl FnMut(u64)) -> Result<(), String> {
    let part = target.with_extension("part");
    let mut hasher = Sha256::new();
    // A part from an earlier attempt: hash what is there and ask for the rest.
    let mut have = match File::open(&part) {
        Ok(mut file) => copy_into(&mut file, &mut hasher, |_| {})?,
        Err(_) => 0,
    };
    if have > f.size {
        let _ = fs::remove_file(&part);
        hasher = Sha256::new();
        have = 0;
    }
    let url = format!("{REPO}/{REVISION}/{}", f.name);
    let mut req = agent.get(&url);
    if have > 0 {
        req = req.header("Range", format!("bytes={have}-"));
    }
    let resp = req.call().map_err(|e| format!("{}: {e}", f.name))?;
    if have > 0 && resp.status().as_u16() != 206 {
        // The server sends the whole file: start over.
        hasher = Sha256::new();
        have = 0;
    }
    let mut out = OpenOptions::new().create(true).write(true).append(have > 0).truncate(have == 0).open(&part).map_err(|e| format!("{}: {e}", f.name))?;
    let mut body = resp.into_body().into_reader();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = body.read(&mut buf).map_err(|e| format!("{}: {e}", f.name))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", f.name))?;
        hasher.update(&buf[..n]);
        have += n as u64;
        progress(have);
        if have > f.size {
            break;
        }
    }
    out.flush().map_err(|e| format!("{}: {e}", f.name))?;
    drop(out);
    let sha: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if have != f.size || sha != f.sha256 {
        let _ = fs::remove_file(&part);
        return Err(format!("{}: {have} bytes with another checksum, discarded", f.name));
    }
    fs::rename(&part, target).map_err(|e| format!("{}: {e}", f.name))
}

fn copy_into(r: &mut impl Read, hasher: &mut Sha256, mut each: impl FnMut(u64)) -> Result<u64, String> {
    let mut buf = vec![0u8; 1 << 20];
    let mut n_total = 0u64;
    loop {
        let n = r.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(n_total);
        }
        hasher.update(&buf[..n]);
        n_total += n as u64;
        each(n_total);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_needs_every_file_at_its_size() {
        let dir = std::env::temp_dir().join(format!("lyrichub-align-model-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert!(!ready(&dir));
        fs::write(dir.join("vocab.json"), vec![0u8; FILES[0].size as usize]).unwrap();
        assert!(!ready(&dir));
        let f = File::create(dir.join("aligner-int8.onnx")).unwrap();
        f.set_len(FILES[1].size).unwrap();
        assert!(ready(&dir));
        f.set_len(FILES[1].size - 1).unwrap();
        assert!(!ready(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Downloads the small file from huggingface.co. Network, so not in the normal run.
    #[test]
    #[ignore]
    fn downloads_and_checks_the_vocab() {
        let dir = std::env::temp_dir().join(format!("lyrichub-align-dl-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build())
            .build()
            .into();
        fs::create_dir_all(&dir).unwrap();
        // A broken part from an "earlier attempt": the first 100 bytes are resumed after it.
        let target = dir.join("vocab.json");
        fetch(&agent, &FILES[0], &target, |_| {}).unwrap();
        let whole = fs::read(&target).unwrap();
        fs::remove_file(&target).unwrap();
        fs::write(target.with_extension("part"), &whole[..100]).unwrap();
        fetch(&agent, &FILES[0], &target, |_| {}).unwrap();
        assert_eq!(fs::read(&target).unwrap(), whole);
        let _ = fs::remove_dir_all(&dir);
    }
}
