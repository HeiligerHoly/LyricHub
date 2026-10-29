//! JSON-RPC 2.0 to LyricHub, one message per line on stdout, shared by the threads.

use std::io::Write;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

#[derive(Clone, Default)]
pub struct Out(Arc<Mutex<()>>);

impl Out {
    pub fn send(&self, message: &Value) {
        let _guard = self.0.lock().unwrap();
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{message}");
        let _ = out.flush();
    }

    pub fn notify(&self, method: &str, params: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// A line for LyricHub's log. Never song names or lyrics, only counts and reasons.
    pub fn log(&self, level: &str, text: impl Into<String>) {
        self.notify("log", json!({ "level": level, "text": text.into() }));
    }
}

