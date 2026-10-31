//! Test plugin for LyricHub's plugin host (PLAN-plugins.md, P1 and P2). It answers `initialize`,
//! reports a status, writes one stderr line and ends on `shutdown` or when its input closes. For
//! every `lyrics` it sends `word_times`: the units of each line spread evenly over the first four
//! fifths of the line, which is easy to tell from LyricHub's own estimate.
//!
//! The host's tests steer it with a `behavior.json` next to the program:
//! `{ "api": 2 }` answers with another protocol version, `{ "crash_ms": 100 }` ends with exit
//! code 3 after that time, `{ "silent": true }` never answers `initialize`, `{ "alloc_mb": 3500 }`
//! asks for that much memory after `initialize`.

use std::io::{BufRead, Write};
use std::time::Duration;

use serde_json::{Value, json};

fn send(message: Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{message}");
    let _ = out.flush();
}

fn word_times(lyrics: &Value) -> Value {
    let empty = Vec::new();
    let lines: Vec<Value> = lyrics["lines"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let n = line["units"].as_array().map_or(0, Vec::len) as u64;
            let start = line["start_ms"].as_u64()?;
            let end = line["end_ms"].as_u64().unwrap_or(start + 3000);
            if n == 0 || end <= start {
                return None;
            }
            let each = (end - start) * 4 / 5 / n;
            let words: Vec<[u64; 2]> = (0..n).map(|i| [start + i * each, start + (i + 1) * each]).collect();
            Some(json!({ "index": index, "words": words }))
        })
        .collect();
    json!({ "jsonrpc": "2.0", "method": "word_times", "params": {
        "key": lyrics["key"], "lyrics_hash": lyrics["lyrics_hash"], "lines": lines,
    }})
}

fn main() {
    let behavior: Value = std::fs::read_to_string("behavior.json")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}));
    eprintln!("example plugin started");
    if let Some(ms) = behavior.get("crash_ms").and_then(Value::as_u64) {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(ms));
            eprintln!("example plugin crashed on purpose");
            std::process::exit(3);
        });
    }
    let api = behavior.get("api").and_then(Value::as_u64).unwrap_or(1);
    let silent = behavior.get("silent").and_then(Value::as_bool).unwrap_or(false);

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        match msg.get("method").and_then(Value::as_str) {
            Some("initialize") if !silent => {
                send(json!({ "jsonrpc": "2.0", "id": msg["id"], "result": { "api": api, "capabilities": ["word_timing"] } }));
                send(json!({ "jsonrpc": "2.0", "method": "status", "params": { "text": "ready" } }));
                send(json!({ "jsonrpc": "2.0", "method": "log", "params": { "level": "info", "text": "initialized" } }));
                if let Some(mb) = behavior.get("alloc_mb").and_then(Value::as_u64) {
                    // Over LyricHub's limit this aborts with "memory allocation of ... failed".
                    let block = vec![1u8; mb as usize * 1024 * 1024];
                    eprintln!("holding {} MB", block.len() >> 20);
                    std::mem::forget(block);
                }
            }
            Some("lyrics") => send(word_times(&msg["params"])),
            Some("shutdown") => break,
            _ => {}
        }
    }
}
