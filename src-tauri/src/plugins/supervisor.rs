//! Keeps one plugin running (PLAN-plugins.md 4.4): start, handshake, restart after a crash with
//! 2 s, 10 s and 60 s pauses, then give up; on stop, `shutdown` and 2 s of grace before the
//! process is ended. Independent of Tauri, like process.rs.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::manifest::API_VERSION;
use super::process::{Event, Process, restart_delay};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);
const WATCH_EVERY: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Starting,
    Running,
    /// Crashed and waiting for the next start.
    Restarting { attempt: u32, in_s: u64 },
    /// Speaks another protocol version.
    Incompatible(String),
    /// Ended too often, or would not start or answer: stays off until switched on again.
    Failed(String),
}

pub struct Config {
    pub exe: PathBuf,
    pub dir: PathBuf,
    /// Parameters of `initialize`.
    pub init: Value,
}

pub type OnState = Arc<dyn Fn(State) + Send + Sync>;
pub type OnEvent = Arc<dyn Fn(Event) + Send + Sync>;

pub struct Supervisor {
    stop: Arc<AtomicBool>,
    process: Arc<Mutex<Option<Arc<Process>>>>,
    thread: Option<JoinHandle<()>>,
}

impl Supervisor {
    pub fn start(config: Config, on_state: OnState, on_event: OnEvent) -> Supervisor {
        let stop = Arc::new(AtomicBool::new(false));
        let process: Arc<Mutex<Option<Arc<Process>>>> = Arc::default();
        let thread = {
            let (stop, process) = (stop.clone(), process.clone());
            std::thread::Builder::new()
                .name("plugin".into())
                .spawn(move || run(config, &stop, &process, &on_state, on_event))
                .ok()
        };
        Supervisor { stop, process, thread }
    }

    /// The running process, for notifications to the plugin (from P2 on).
    #[cfg(test)]
    pub fn process(&self) -> Option<Arc<Process>> {
        self.process.lock().unwrap().clone()
    }

    /// Asks the plugin to end, waits up to 2 s, then ends it.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(p) = self.process.lock().unwrap().take() {
            p.notify("shutdown", Value::Null);
            let since = Instant::now();
            while p.exited().is_none() && since.elapsed() < SHUTDOWN_GRACE {
                std::thread::sleep(Duration::from_millis(50));
            }
            p.kill();
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(
    config: Config,
    stop: &AtomicBool,
    slot: &Mutex<Option<Arc<Process>>>,
    on_state: &OnState,
    on_event: OnEvent,
) {
    // The last stderr line explains a crash on the plugin page.
    let last_stderr: Arc<Mutex<Option<String>>> = Arc::default();
    let events: OnEvent = {
        let last_stderr = last_stderr.clone();
        Arc::new(move |e: Event| {
            if let Event::Stderr(line) = &e {
                *last_stderr.lock().unwrap() = Some(line.clone());
            }
            on_event(e)
        })
    };
    let mut restarts = 0;
    while !stop.load(Ordering::SeqCst) {
        on_state(State::Starting);
        let process = match Process::spawn(&config.exe, &config.dir, events.clone()) {
            Ok(p) => Arc::new(p),
            Err(e) => {
                on_state(State::Failed(format!("cannot start {}: {e}", config.exe.display())));
                return;
            }
        };
        *slot.lock().unwrap() = Some(process.clone());
        match process.request("initialize", config.init.clone(), HANDSHAKE_TIMEOUT) {
            Ok(answer) => {
                let api = answer.get("api").and_then(Value::as_u64).unwrap_or(0);
                if api != u64::from(API_VERSION) {
                    process.kill();
                    on_state(State::Incompatible(format!("plugin speaks protocol {api}, LyricHub {API_VERSION}")));
                    return;
                }
            }
            Err(e) if !stop.load(Ordering::SeqCst) => {
                process.kill();
                on_state(State::Failed(e));
                return;
            }
            Err(_) => return,
        }
        on_state(State::Running);
        let code = loop {
            if stop.load(Ordering::SeqCst) {
                return; // stop() ends the process.
            }
            if let Some(code) = process.exited() {
                break code;
            }
            std::thread::sleep(WATCH_EVERY);
        };
        slot.lock().unwrap().take();
        let why = last_stderr.lock().unwrap().take().unwrap_or_else(|| match code {
            Some(c) => format!("ended with exit code {c}"),
            None => "ended".into(),
        });
        tracing::warn!(exe = %config.exe.display(), ?code, reason = %why, "plugin ended unexpectedly");
        let Some(delay) = restart_delay(restarts) else {
            on_state(State::Failed(why));
            return;
        };
        restarts += 1;
        on_state(State::Restarting { attempt: restarts, in_s: delay.as_secs() });
        let since = Instant::now();
        while since.elapsed() < delay {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Runs the example plugin (plugins/example) through the supervisor. It is built first, so
/// this takes a while: `cargo test --lib example_plugin -- --ignored --nocapture`.
#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::*;

    fn example_exe() -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugins/example");
        let status = std::process::Command::new("cargo")
            .args(["build", "--quiet", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .status()
            .expect("cargo");
        assert!(status.success(), "example plugin did not build");
        root.join("target/debug/lyrichub-plugin-example.exe")
    }

    /// A plugin folder with the example program and a `behavior.json` for this case.
    fn plugin_dir(tag: &str, behavior: Value) -> Config {
        let dir = std::env::temp_dir().join(format!("lyrichub-supervisor-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("example.exe");
        std::fs::copy(example_exe(), &exe).unwrap();
        std::fs::write(dir.join("behavior.json"), behavior.to_string()).unwrap();
        Config { exe, dir, init: json!({ "api": API_VERSION }) }
    }

    type Collected = (Arc<Mutex<Vec<State>>>, OnState, Arc<Mutex<Vec<Event>>>, OnEvent);

    fn collect() -> Collected {
        let states: Arc<Mutex<Vec<State>>> = Arc::default();
        let events: Arc<Mutex<Vec<Event>>> = Arc::default();
        let (s, e) = (states.clone(), events.clone());
        (states, Arc::new(move |x| s.lock().unwrap().push(x)), events, Arc::new(move |x| e.lock().unwrap().push(x)))
    }

    fn wait_for(states: &Mutex<Vec<State>>, pred: impl Fn(&State) -> bool, secs: u64) -> bool {
        let since = Instant::now();
        while since.elapsed() < Duration::from_secs(secs) {
            if states.lock().unwrap().iter().any(&pred) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[test]
    #[ignore]
    fn example_plugin_runs_reports_and_stops() {
        let (states, on_state, events, on_event) = collect();
        let sup = Supervisor::start(plugin_dir("run", json!({})), on_state, on_event);
        assert!(wait_for(&states, |s| *s == State::Running, 10), "{:?}", states.lock().unwrap());
        std::thread::sleep(Duration::from_millis(300));
        assert!(events.lock().unwrap().iter().any(|e| matches!(e, Event::Status { .. })), "status expected");
        let pid = sup.process().unwrap().pid();
        sup.stop();
        assert!(!std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false));
    }

    #[test]
    #[ignore]
    fn a_crashing_plugin_is_restarted_and_then_given_up() {
        let (states, on_state, _, on_event) = collect();
        let sup = Supervisor::start(plugin_dir("crash", json!({ "crash_ms": 100 })), on_state, on_event);
        assert!(wait_for(&states, |s| matches!(s, State::Restarting { attempt: 1, in_s: 2 }), 10));
        assert!(wait_for(&states, |s| matches!(s, State::Restarting { attempt: 2, in_s: 10 }), 15));
        // The third pause is 60 s and then it gives up (restart_delay is tested on its own).
        sup.stop();
    }

    #[test]
    #[ignore]
    fn another_protocol_version_or_silence_keeps_it_off() {
        let (states, on_state, _, on_event) = collect();
        let sup = Supervisor::start(plugin_dir("api", json!({ "api": 2 })), on_state, on_event);
        assert!(wait_for(&states, |s| matches!(s, State::Incompatible(_)), 10), "{:?}", states.lock().unwrap());
        sup.stop();
        let (states, on_state, _, on_event) = collect();
        let sup = Supervisor::start(plugin_dir("silent", json!({ "silent": true })), on_state, on_event);
        assert!(wait_for(&states, |s| matches!(s, State::Failed(_)), 10), "{:?}", states.lock().unwrap());
        sup.stop();
    }
}
