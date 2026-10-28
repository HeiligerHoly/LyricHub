//! Test plugin for LyricHub's plugin host (PLAN-plugins.md, P1). It answers `initialize`, reports
//! a status, writes one stderr line and ends on `shutdown` or when its input closes.
//!
//! The host's tests steer it with a `behavior.json` next to the program:
//! `{ "api": 2 }` answers with another protocol version, `{ "crash_ms": 100 }` ends with exit
//! code 3 after that time, `{ "silent": true }` never answers `initialize`.

use std::io::{BufRead, Write};
use std::time::Duration;

use serde_json::{Value, json};

fn send(message: Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{message}");
    let _ = out.flush();
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
                send(json!({ "jsonrpc": "2.0", "id": msg["id"], "result": { "api": api, "capabilities": [] } }));
                send(json!({ "jsonrpc": "2.0", "method": "status", "params": { "text": "ready" } }));
                send(json!({ "jsonrpc": "2.0", "method": "log", "params": { "level": "info", "text": "initialized" } }));
            }
            Some("shutdown") => break,
            _ => {}
        }
    }
}
