//! One plugin process (PLAN-plugins.md 4.1, 4.4). It starts without a console window at
//! below-normal priority and joins a job object that ends it together with LyricHub, even if
//! LyricHub crashes, and caps its memory (IDEEN.md, package A). Messages are JSON-RPC 2.0, one
//! per line, over stdin/stdout; stderr lines go to the log. Nothing here knows Tauri, so the
//! host logic can be tested on its own.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::{Value, json};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
/// Committed memory one plugin may hold. Recognize peaked at 1.75 GB committed (1.2 GB working
/// set) on 2026-10-31; a plugin that asks for more gets no memory, ends, and is restarted.
const PROCESS_MEMORY_LIMIT: usize = 3 << 30;
/// Longer lines are dropped; word times for a whole song stay far below this.
const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
const TEXT_MAX_CHARS: usize = 300;

/// What a plugin tells the host besides answers to requests.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Short text and optional progress 0..1 for the plugin page.
    Status { text: String, progress: Option<f64> },
    Log { level: String, text: String },
    /// Any other notification, for the capabilities (`word_times` from P2 on).
    Notification { method: String, params: Value },
    Stderr(String),
}

/// One line from the plugin.
#[derive(Debug, PartialEq)]
pub enum Message {
    Response { id: u64, result: Result<Value, String> },
    Notification { method: String, params: Value },
    Invalid(String),
}

pub fn parse_message(line: &str) -> Message {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Message::Invalid("not JSON".into());
    };
    let method = v.get("method").and_then(Value::as_str);
    match (v.get("id").and_then(Value::as_u64), method) {
        (Some(id), None) => {
            let result = match v.get("error") {
                Some(e) => Err(e.get("message").and_then(Value::as_str).unwrap_or("error").to_owned()),
                None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
            };
            Message::Response { id, result }
        }
        // Requests from a plugin to the host do not exist in protocol version 1.
        (Some(_), Some(m)) => Message::Invalid(format!("unsupported request {m}")),
        (None, Some(m)) => {
            Message::Notification { method: m.to_owned(), params: v.get("params").cloned().unwrap_or(Value::Null) }
        }
        (None, None) => Message::Invalid("neither answer nor notification".into()),
    }
}

/// `status` and `log` become their own events; text is cut to a sane length.
fn to_event(method: String, params: Value) -> Event {
    let text = |key: &str| params.get(key).and_then(Value::as_str).map(short).unwrap_or_default();
    match method.as_str() {
        "status" => Event::Status {
            text: text("text"),
            progress: params.get("progress").and_then(Value::as_f64).filter(|p| p.is_finite()).map(|p| p.clamp(0.0, 1.0)),
        },
        "log" => Event::Log { level: text("level"), text: text("text") },
        _ => Event::Notification { method, params },
    }
}

fn short(s: &str) -> String {
    let mut out: String = s.chars().filter(|c| !c.is_control()).take(TEXT_MAX_CHARS).collect();
    if s.chars().count() > TEXT_MAX_CHARS {
        out.push('…');
    }
    out
}

/// Pause before the n-th restart after a crash (PLAN-plugins.md 4.4); `None` after three.
pub fn restart_delay(restarts: u32) -> Option<Duration> {
    [2, 10, 60].get(restarts as usize).map(|&s| Duration::from_secs(s))
}

type Pending = Arc<Mutex<HashMap<u64, mpsc::Sender<Result<Value, String>>>>>;

pub struct Process {
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Pending,
    next_id: AtomicU64,
}

impl Process {
    pub fn spawn(exe: &Path, dir: &Path, on_event: Arc<dyn Fn(Event) + Send + Sync>) -> std::io::Result<Process> {
        let mut child = Command::new(exe)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS)
            .spawn()?;
        assign_to_job(&child);
        let pending: Pending = Arc::default();
        if let Some(stdout) = child.stdout.take() {
            let (pending, on_event) = (pending.clone(), on_event.clone());
            std::thread::Builder::new().name("plugin-out".into()).spawn(move || {
                for line in BufReader::new(stdout).split(b'\n') {
                    let Ok(line) = line else { break };
                    if line.len() > MAX_LINE_BYTES {
                        on_event(Event::Log { level: "warn".into(), text: "message too long, dropped".into() });
                        continue;
                    }
                    match parse_message(String::from_utf8_lossy(&line).trim()) {
                        Message::Response { id, result } => {
                            if let Some(tx) = pending.lock().unwrap().remove(&id) {
                                let _ = tx.send(result);
                            }
                        }
                        Message::Notification { method, params } => on_event(to_event(method, params)),
                        Message::Invalid(why) => on_event(Event::Log { level: "warn".into(), text: why }),
                    }
                }
            })?;
        }
        if let Some(stderr) = child.stderr.take() {
            std::thread::Builder::new().name("plugin-err".into()).spawn(move || {
                for line in BufReader::new(stderr).lines() {
                    let Ok(line) = line else { break };
                    if !line.trim().is_empty() {
                        on_event(Event::Stderr(short(line.trim())));
                    }
                }
            })?;
        }
        let stdin = child.stdin.take();
        Ok(Process { child: Mutex::new(child), stdin: Mutex::new(stdin), pending, next_id: AtomicU64::new(1) })
    }

    fn send(&self, message: &Value) -> Result<(), String> {
        let mut stdin = self.stdin.lock().unwrap();
        let pipe = stdin.as_mut().ok_or("the plugin's input is closed")?;
        writeln!(pipe, "{message}").and_then(|()| pipe.flush()).map_err(|e| format!("cannot write to the plugin: {e}"))
    }

    pub fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        if let Err(e) = self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })) {
            self.pending.lock().unwrap().remove(&id);
            return Err(e);
        }
        rx.recv_timeout(timeout).unwrap_or_else(|_| {
            self.pending.lock().unwrap().remove(&id);
            Err(format!("no answer to {method} within {} s", timeout.as_secs()))
        })
    }

    pub fn notify(&self, method: &str, params: Value) {
        if let Err(e) = self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params })) {
            tracing::debug!(error = %e, method, "plugin notification not sent");
        }
    }

    /// `Some(exit code)` once the process has ended.
    pub fn exited(&self) -> Option<Option<i32>> {
        match self.child.lock().unwrap().try_wait() {
            Ok(Some(status)) => Some(status.code()),
            Ok(None) => None,
            Err(_) => Some(None),
        }
    }

    pub fn kill(&self) {
        self.stdin.lock().unwrap().take();
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
    }

    #[cfg(test)]
    pub fn pid(&self) -> u32 {
        self.child.lock().unwrap().id()
    }
}

/// A job object that ends its processes when its last handle closes, that is when LyricHub
/// ends, and gives each of them at most `memory_limit` bytes of committed memory.
fn plugin_job(memory_limit: usize) -> Option<HANDLE> {
    // SAFETY: plain object creation; the structure is fully initialised.
    unsafe {
        let job = CreateJobObjectW(None, None).ok()?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        info.ProcessMemoryLimit = memory_limit;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const core::ffi::c_void,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .ok()?;
        Some(job)
    }
}

fn assign_to_job(child: &Child) {
    // The handle stays open for the life of LyricHub; Windows closes it at exit.
    static JOB: OnceLock<Option<isize>> = OnceLock::new();
    let job = JOB.get_or_init(|| plugin_job(PROCESS_MEMORY_LIMIT).map(|h| h.0 as isize));
    let Some(job) = job else {
        tracing::warn!("no job object: plugins may outlive LyricHub");
        return;
    };
    // SAFETY: both handles are valid; the child's handle lives as long as `child`.
    if let Err(e) = unsafe { AssignProcessToJobObject(HANDLE(*job as *mut _), HANDLE(child.as_raw_handle())) } {
        tracing::warn!(error = %e, "plugin not in the job object: it may outlive LyricHub");
    }
}

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::CloseHandle;

    use super::*;

    #[test]
    fn messages_are_answers_or_notifications() {
        assert_eq!(
            parse_message(r#"{"jsonrpc":"2.0","id":3,"result":{"api":1}}"#),
            Message::Response { id: 3, result: Ok(json!({ "api": 1 })) }
        );
        assert_eq!(
            parse_message(r#"{"jsonrpc":"2.0","id":4,"error":{"code":-1,"message":"nope"}}"#),
            Message::Response { id: 4, result: Err("nope".into()) }
        );
        assert!(matches!(parse_message(r#"{"jsonrpc":"2.0","method":"status","params":{}}"#), Message::Notification { .. }));
        assert!(matches!(parse_message(r#"{"jsonrpc":"2.0","id":1,"method":"ask"}"#), Message::Invalid(_)));
        assert!(matches!(parse_message("no json"), Message::Invalid(_)));
    }

    #[test]
    fn status_and_log_are_cut_and_clamped() {
        let long = "x".repeat(500);
        let Event::Status { text, progress } =
            to_event("status".into(), json!({ "text": format!("{long}\u{7}"), "progress": 3.0 }))
        else {
            panic!("status expected");
        };
        assert_eq!((text.chars().count(), progress), (TEXT_MAX_CHARS + 1, Some(1.0)));
        assert!(matches!(to_event("log".into(), json!({ "level": "info", "text": "hi" })), Event::Log { .. }));
        assert!(matches!(to_event("word_times".into(), json!({})), Event::Notification { .. }));
    }

    #[test]
    fn three_restarts_then_none() {
        let delays: Vec<_> = (0..4).map(restart_delay).collect();
        assert_eq!(delays, [Some(Duration::from_secs(2)), Some(Duration::from_secs(10)), Some(Duration::from_secs(60)), None]);
    }

    /// Closing the job's last handle ends its processes, as when LyricHub ends or crashes.
    #[test]
    fn a_process_in_the_job_ends_when_the_job_closes() {
        let job = plugin_job(PROCESS_MEMORY_LIMIT).expect("job object");
        let mut child = Command::new("powershell")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .expect("powershell");
        // SAFETY: valid handles owned by this test.
        unsafe {
            AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())).expect("assign");
            CloseHandle(job).unwrap();
        }
        let since = std::time::Instant::now();
        while child.try_wait().unwrap().is_none() {
            assert!(since.elapsed() < Duration::from_secs(5), "process outlived its job");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// A process that asks for more than the job allows gets no memory; without the job the same
    /// request succeeds.
    #[test]
    fn a_process_in_the_job_gets_no_more_than_the_limit() {
        let exit_code = |limit: Option<usize>| {
            let mut child = Command::new("powershell")
                .args(["-NoProfile", "-Command", "$ErrorActionPreference = 'Stop'; $a = [byte[]]::new(1GB); exit 0"])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .expect("powershell");
            let job = limit.map(|l| plugin_job(l).expect("job object"));
            if let Some(job) = job {
                // SAFETY: valid handles owned by this test; PowerShell starts long before it allocates.
                unsafe { AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())).expect("assign") };
            }
            let code = child.wait().unwrap().code();
            if let Some(job) = job {
                // SAFETY: the job handle is owned by this test.
                unsafe { CloseHandle(job).unwrap() };
            }
            code
        };
        assert_eq!(exit_code(None), Some(0), "1 GB without a limit");
        assert_ne!(exit_code(Some(512 << 20)), Some(0), "1 GB under a 512 MB limit");
    }
}
