//! OBS browser source (IDEEN.md, package D): a small HTTP server on 127.0.0.1 that serves the
//! overlay page itself, built into LyricHub, with its data as JSON and server-sent events, so a
//! stream can show the lyrics as their own layer. While it runs, `now-playing.txt` in LyricHub's
//! data folder holds "Title – Artist" for a text source. Off by default: lyrics in a stream are
//! shown to an audience. The page and its data need the token from the settings; the Host header
//! must name 127.0.0.1 or localhost, so no web page can reach the server through DNS tricks.

use std::io::Write;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Listener, Manager};
use tiny_http::{Header, Request, Response, Server};

use crate::app_state::NowPlayingState;
use crate::settings::SettingsState;

/// Events the overlay page listens to; "overlay-layout" stays out, it describes the desktop window.
const EVENTS: [&str; 8] = ["track", "track-changing", "playback", "clock", "lyrics", "nudge", "notice", "settings"];
const KEEPALIVE: Duration = Duration::from_secs(15);
/// Content security for the served page: only its own files and the server's data.
const CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'";

static CLIENTS: Mutex<Vec<Sender<String>>> = Mutex::new(Vec::new());

fn broadcast(event: &str, payload: &str) {
    let message = format!("event: {event}\ndata: {payload}\n\n");
    CLIENTS.lock().unwrap().retain(|c| c.send(message.clone()).is_ok());
}

pub fn now_playing_file() -> std::path::PathBuf {
    crate::paths::data_dir().join("now-playing.txt")
}

/// Starts the watcher: it runs the server while the setting is on, on the port it names.
pub fn spawn(app: AppHandle) {
    for event in EVENTS {
        let app2 = app.clone();
        app.listen_any(event, move |e| {
            if !CLIENTS.lock().unwrap().is_empty() {
                broadcast(event, e.payload());
            }
            if event == "track" {
                write_now_playing(&app2);
            }
        });
    }
    let _ = std::thread::Builder::new().name("stream".into()).spawn(move || watch(&app));
}

fn watch(app: &AppHandle) {
    let mut running: Option<(u16, Arc<Server>, std::thread::JoinHandle<()>)> = None;
    loop {
        let s = app.state::<SettingsState>().get().stream;
        let wanted = s.enabled.then_some(s.port);
        if running.as_ref().map(|r| r.0) != wanted {
            if let Some((port, server, thread)) = running.take() {
                server.unblock();
                let _ = thread.join();
                CLIENTS.lock().unwrap().clear();
                let _ = std::fs::remove_file(now_playing_file());
                tracing::info!(port, "stream server stopped");
            }
            if let Some(port) = wanted {
                if s.token.is_empty() {
                    let token = new_token();
                    app.state::<SettingsState>().update(|s| s.stream.token = token);
                }
                match Server::http(("127.0.0.1", port)) {
                    Ok(server) => {
                        let server = Arc::new(server);
                        let (srv, app2) = (server.clone(), app.clone());
                        let thread = std::thread::spawn(move || serve(&app2, &srv, port));
                        running = Some((port, server, thread));
                        write_now_playing(app);
                        tracing::info!(port, "stream server started");
                    }
                    Err(e) => {
                        tracing::warn!(port, error = %e, "stream server could not start");
                        // Try again only when the settings change.
                        while app.state::<SettingsState>().get().stream == s {
                            std::thread::sleep(Duration::from_secs(1));
                        }
                        continue;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn new_token() -> String {
    let mut bytes = [0u8; 16];
    let _ = getrandom::fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn write_now_playing(app: &AppHandle) {
    if !app.state::<SettingsState>().get().stream.enabled {
        return;
    }
    let now = app.state::<NowPlayingState>().snapshot();
    let text = now.track.map(|t| format!("{} – {}", t.title, t.artist)).unwrap_or_default();
    let _ = std::fs::write(now_playing_file(), text);
}

fn serve(app: &AppHandle, server: &Server, port: u16) {
    for request in server.incoming_requests() {
        let app = app.clone();
        // Each request on its own thread: the event stream stays open for the page's lifetime.
        std::thread::spawn(move || handle(&app, request, port));
    }
}

/// Path and query parameters of a request URL.
fn split(url: &str) -> (&str, Vec<(&str, &str)>) {
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    (path, query.split('&').filter_map(|kv| kv.split_once('=')).collect())
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header")
}

fn json_response(value: &Value) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(value.to_string().into_bytes()).with_header(header("Content-Type", "application/json; charset=utf-8"))
}

fn status(code: u16) -> Response<std::io::Empty> {
    Response::empty(code)
}

fn handle(app: &AppHandle, request: Request, port: u16) {
    let host_ok = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Host"))
        .is_some_and(|h| [format!("127.0.0.1:{port}"), format!("localhost:{port}")].contains(&h.value.as_str().to_ascii_lowercase()));
    if !host_ok {
        let _ = request.respond(status(403));
        return;
    }
    let url = request.url().to_owned();
    let (path, query) = split(&url);
    let param = |name: &str| query.iter().find(|(k, _)| *k == name).map(|(_, v)| *v);
    let token = app.state::<SettingsState>().get().stream.token;
    let authorized = !token.is_empty() && param("token") == Some(token.as_str());
    match path {
        "/" | "/overlay" | "/overlay/" if authorized => {
            // The settings window and the overlay are one single-page app; the page routes itself.
            match app.asset_resolver().get("index.html".into()) {
                Some(asset) => {
                    let _ = request.respond(
                        Response::from_data(asset.bytes)
                            .with_header(header("Content-Type", "text/html; charset=utf-8"))
                            .with_header(header("Content-Security-Policy", CSP))
                            .with_header(header("Cache-Control", "no-store")),
                    );
                }
                None => {
                    let _ = request.respond(status(404));
                }
            }
        }
        "/api/events" if authorized => events(request),
        p if p.starts_with("/api/") && authorized => {
            let value = data(app, &p["/api/".len()..], param("width"), param("scale"));
            let _ = match value {
                Some(v) => request.respond(json_response(&v)),
                None => request.respond(Response::from_data(b"null".to_vec()).with_header(header("Content-Type", "application/json"))),
            };
        }
        p if p.starts_with("/api/") || p == "/" || p.starts_with("/overlay") => {
            let _ = request.respond(status(403));
        }
        // The app's own files: scripts, styles, fonts. They hold no secrets.
        p => match app.asset_resolver().get(p.trim_start_matches('/').into()) {
            Some(asset) if !p.contains("..") => {
                let _ = request.respond(Response::from_data(asset.bytes).with_header(header("Content-Type", &asset.mime_type)));
            }
            _ => {
                let _ = request.respond(status(404));
            }
        },
    }
}

/// The data behind the page's read commands.
fn data(app: &AppHandle, command: &str, width: Option<&str>, scale: Option<&str>) -> Option<Value> {
    let state = app.state::<NowPlayingState>();
    Some(match command {
        "get_now_playing" => serde_json::to_value(state.snapshot()).ok()?,
        "get_lyrics" => serde_json::to_value(state.lyrics()?).ok()?,
        "get_clock" => serde_json::to_value(crate::app_state::clock_event(app, "query")).ok()?,
        "get_settings" => serde_json::to_value(app.state::<SettingsState>().get()).ok()?,
        "get_overlay_layout" => {
            // The browser source decides the size: the layout from the settings, the page width
            // from the source, the text size from `scale` in the URL.
            let width: f64 = width.and_then(|w| w.parse().ok()).filter(|w: &f64| w.is_finite() && *w > 100.0).unwrap_or(800.0);
            let scale: f64 = scale.and_then(|s| s.parse().ok()).filter(|s: &f64| s.is_finite() && *s > 0.2).unwrap_or(1.5);
            let layout = app.state::<SettingsState>().get().overlay.layout;
            json!({
                "seq": 0, "layout": layout, "width_css": width, "max_width_css": width, "ui_scale": scale,
                "align": "center", "preview": false, "show_seq": 0, "edit": null,
            })
        }
        _ => return None,
    })
}

/// The next piece of the event stream: an event, or a comment now and then so browsers and OBS
/// keep the connection; `None` once the server stops.
fn next_message(rx: &Receiver<String>) -> Option<String> {
    match rx.recv_timeout(KEEPALIVE) {
        Ok(m) => Some(m),
        Err(RecvTimeoutError::Timeout) => Some(": keep-alive\n\n".into()),
        Err(RecvTimeoutError::Disconnected) => None,
    }
}

/// Server-sent events until the page goes away. tiny_http sends a body of unknown length in
/// 8 KB pieces, too late for events, so this stream is written by hand on the raw connection.
fn events(request: Request) {
    let (tx, rx) = mpsc::channel();
    CLIENTS.lock().unwrap().push(tx);
    let mut out = request.into_writer();
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n: hello\n\n";
    let mut send = |text: &str| out.write_all(text.as_bytes()).and_then(|()| out.flush()).is_ok();
    if !send(head) {
        return;
    }
    while let Some(message) = next_message(&rx) {
        if !send(&message) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_split_into_path_and_parameters() {
        let (path, q) = split("/api/get_overlay_layout?token=ab12&width=1280&scale=2");
        assert_eq!(path, "/api/get_overlay_layout");
        assert_eq!(q, [("token", "ab12"), ("width", "1280"), ("scale", "2")]);
        assert_eq!(split("/overlay"), ("/overlay", vec![]));
    }

    #[test]
    fn tokens_are_32_hex_characters_and_differ() {
        let (a, b) = (new_token(), new_token());
        assert!(a.len() == 32 && a.chars().all(|c| c.is_ascii_hexdigit()) && a != b);
    }

    #[test]
    fn the_event_stream_hands_out_messages_and_ends_with_the_sender() {
        let (tx, rx) = mpsc::channel();
        tx.send("event: x\ndata: 1\n\n".to_owned()).unwrap();
        drop(tx);
        assert_eq!(next_message(&rx).as_deref(), Some("event: x\ndata: 1\n\n"));
        assert_eq!(next_message(&rx), None);
    }
}
