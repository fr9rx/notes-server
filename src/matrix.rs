//! Server status on the Arduino UNO Q's 8×13 LED matrix.
//!
//! The matrix is driven by the board's STM32U585 running the native Zephyr
//! firmware in `mcu/notes-matrix/`. Linux reaches it through `arduino-router`,
//! which owns the UART between the two chips and routes MessagePack-RPC
//! messages between its clients. The firmware registers these methods with
//! the router, and this module calls them over the router's Unix socket
//! (`/var/run/arduino-router.sock`):
//!
//! - `notes/status` notification `[state, requests, uploads]`, once a second.
//!   `state` is `"B"` (starting), `"O"` (ok), `"W"` (warning), `"E"` (error) or
//!   `"D"` (stopping); the counts cover the last second.
//! - `notes/text` notification `[text]`: scroll `text` once (A-Z, 0-9, space and `.:-/!%`).
//! - `notes/hello` request `[]` → `"notes-matrix <version>"`, to confirm the
//!   firmware is there.
//!
//! No Arduino Bridge library is involved on either side: both ends speak
//! MessagePack-RPC directly. The router and everything else using it (App Lab,
//! the cloud connector) keep working.
//!
//! The STM32 shows "server down" if no status arrives for 5 s, so a crashed or
//! hung server is visible even though it can no longer say so. The link can
//! never slow the web server down: messages go through a bounded queue and are
//! dropped when the link is busy or absent, and the link thread reconnects on
//! its own (for example when the router restarts).

use std::io::{self, Write};
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::time::{Duration, Instant};

use rmpv::Value;

use crate::AppState;

const RECONNECT_DELAY: Duration = Duration::from_secs(5);
const HELLO_RETRY: Duration = Duration::from_secs(30);
const MAX_TEXT_CHARS: usize = 48;

const REQUEST: u64 = 0;
const RESPONSE: u64 = 1;
const NOTIFICATION: u64 = 2;

/// Request/upload counters, drained once a second by [`report`].
#[derive(Debug, Default)]
pub struct Metrics {
    requests: AtomicU64,
    uploads: AtomicU64,
}

impl Metrics {
    pub fn record_request(&self) {
        self.requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_uploads(&self, images: usize) {
        self.uploads.fetch_add(images as u64, Ordering::Relaxed);
    }

    /// Returns and resets `(requests, uploaded images)`.
    pub fn take(&self) -> (u64, u64) {
        (
            self.requests.swap(0, Ordering::Relaxed),
            self.uploads.swap(0, Ordering::Relaxed),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Starting,
    Ok,
    /// Uploads are refused because the disk is nearly full.
    Warning,
    /// The database is not answering.
    Error,
    Stopping,
}

impl Status {
    fn code(self) -> &'static str {
        match self {
            Self::Starting => "B",
            Self::Ok => "O",
            Self::Warning => "W",
            Self::Error => "E",
            Self::Stopping => "D",
        }
    }
}

enum Msg {
    Status(Status, u64, u64),
    Text(String),
}

/// Handle to the matrix link; cheap to clone. A disabled handle ignores everything.
#[derive(Clone)]
pub struct Matrix {
    tx: Option<SyncSender<Msg>>,
}

impl Matrix {
    pub fn disabled() -> Self {
        Self { tx: None }
    }

    /// Starts the background link to arduino-router's socket.
    pub fn open(router_socket: PathBuf) -> Self {
        let (tx, rx) = mpsc::sync_channel(32);
        let spawned = std::thread::Builder::new()
            .name("matrix-link".into())
            .spawn(move || run_link(&router_socket, rx));
        match spawned {
            Ok(_) => Self { tx: Some(tx) },
            Err(e) => {
                tracing::warn!(error = %e, "could not start LED matrix thread");
                Self::disabled()
            }
        }
    }

    pub fn status(&self, status: Status, requests: u64, uploads: u64) {
        self.send(Msg::Status(status, requests, uploads));
    }

    pub fn text(&self, text: &str) {
        let text = sanitize_text(text);
        if !text.is_empty() {
            self.send(Msg::Text(text));
        }
    }

    fn send(&self, msg: Msg) {
        if let Some(tx) = &self.tx {
            // Full queue (link stalled) or thread gone: drop it; status is best-effort.
            let _ = tx.try_send(msg);
        }
    }
}

// ---- MessagePack-RPC encoding ---------------------------------------------------

fn encode(value: &Value) -> Vec<u8> {
    let mut buf = Vec::with_capacity(48);
    rmpv::encode::write_value(&mut buf, value).expect("writing to a Vec cannot fail");
    buf
}

fn notification(method: &str, params: Vec<Value>) -> Vec<u8> {
    encode(&Value::Array(vec![
        NOTIFICATION.into(),
        method.into(),
        Value::Array(params),
    ]))
}

fn request(id: u64, method: &str, params: Vec<Value>) -> Vec<u8> {
    encode(&Value::Array(vec![
        REQUEST.into(),
        id.into(),
        method.into(),
        Value::Array(params),
    ]))
}

fn encode_msg(msg: &Msg) -> Vec<u8> {
    match msg {
        Msg::Status(status, requests, uploads) => notification(
            "notes/status",
            vec![
                status.code().into(),
                (*requests).min(9999).into(),
                (*uploads).min(999).into(),
            ],
        ),
        Msg::Text(text) => notification("notes/text", vec![text.as_str().into()]),
    }
}

/// Uppercases and keeps only characters the MCU font can draw.
fn sanitize_text(text: &str) -> String {
    let kept: String = text
        .chars()
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| c.is_ascii_alphanumeric() || " .:-/!%".contains(*c))
        .collect();
    kept.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_TEXT_CHARS)
        .collect()
}

// ---- Link thread --------------------------------------------------------------------

/// Owns the router connection: (re)connects and writes queued messages until
/// every [`Matrix`] handle is dropped.
fn run_link(socket: &Path, rx: Receiver<Msg>) {
    let mut reported_failure = false;
    let mut next_id = 1u64;
    loop {
        let conn = match connect(socket) {
            Ok(conn) => conn,
            Err(e) => {
                if !reported_failure {
                    tracing::warn!(socket = %socket.display(), error = %e,
                        "arduino-router unavailable; LED matrix off, retrying every 5 s");
                    reported_failure = true;
                }
                if !discard_for(&rx, RECONNECT_DELAY) {
                    return;
                }
                continue;
            }
        };
        tracing::info!(socket = %socket.display(), "connected to arduino-router for the LED matrix");
        reported_failure = false;

        let confirmed = Arc::new(AtomicBool::new(false));
        let closed = Arc::new(AtomicBool::new(false));
        spawn_reader(&conn, confirmed.clone(), closed.clone());

        let mut conn = conn;
        let mut last_hello: Option<Instant> = None;
        let ended = loop {
            if closed.load(Ordering::Relaxed) {
                break false;
            }
            if !confirmed.load(Ordering::Relaxed)
                && last_hello.is_none_or(|t| t.elapsed() >= HELLO_RETRY)
            {
                last_hello = Some(Instant::now());
                let hello = request(next_id, "notes/hello", vec![]);
                next_id += 1;
                if let Err(e) = conn.write_all(&hello) {
                    tracing::warn!(error = %e, "arduino-router write failed; reconnecting");
                    break false;
                }
            }
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(msg) => {
                    if let Err(e) = conn.write_all(&encode_msg(&msg)) {
                        tracing::warn!(error = %e, "arduino-router write failed; reconnecting");
                        break false;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break true,
            }
        };
        closed.store(true, Ordering::Relaxed);
        if ended {
            let _ = conn.flush();
            return;
        }
        if !discard_for(&rx, RECONNECT_DELAY) {
            return;
        }
    }
}

/// Drops queued messages for `wait`; returns false if all handles are gone.
fn discard_for(rx: &Receiver<Msg>, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(_) => continue,
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

#[cfg(unix)]
type Conn = std::os::unix::net::UnixStream;

#[cfg(unix)]
fn connect(socket: &Path) -> io::Result<Conn> {
    Conn::connect(socket)
}

/// Off the board (development on Windows): append the MessagePack stream to a file.
#[cfg(not(unix))]
type Conn = std::fs::File;

#[cfg(not(unix))]
fn connect(socket: &Path) -> io::Result<Conn> {
    std::fs::OpenOptions::new().create(true).append(true).open(socket)
}

/// Reads the router's replies: the firmware's answer to `notes/hello`, or an
/// error if it hasn't registered (not flashed, or still booting).
#[cfg(unix)]
fn spawn_reader(conn: &Conn, confirmed: Arc<AtomicBool>, closed: Arc<AtomicBool>) {
    let Ok(read_half) = conn.try_clone() else { return };
    let _ = std::thread::Builder::new()
        .name("matrix-link-rx".into())
        .spawn(move || {
            let mut reader = io::BufReader::new(read_half);
            let mut warned = false;
            while !closed.load(Ordering::Relaxed) {
                let Ok(value) = rmpv::decode::read_value(&mut reader) else {
                    closed.store(true, Ordering::Relaxed);
                    return;
                };
                match parse_response(&value) {
                    Some(Ok(hello)) => {
                        if !confirmed.swap(true, Ordering::Relaxed) {
                            tracing::info!(firmware = %hello, "LED matrix firmware answered");
                        }
                    }
                    Some(Err(error)) if !warned => {
                        tracing::warn!(%error,
                            "LED matrix firmware not reachable via arduino-router (is mcu/notes-matrix flashed?)");
                        warned = true;
                    }
                    _ => {}
                }
            }
        });
}

#[cfg(not(unix))]
fn spawn_reader(_conn: &Conn, _confirmed: Arc<AtomicBool>, _closed: Arc<AtomicBool>) {}

/// `[1, id, error, result]` → `Ok(result as text)` / `Err(error as text)`.
#[cfg_attr(not(unix), allow(dead_code))]
fn parse_response(value: &Value) -> Option<Result<String, String>> {
    let items = value.as_array()?;
    if items.len() != 4 || items[0].as_u64() != Some(RESPONSE) {
        return None;
    }
    let text = |v: &Value| v.as_str().map_or_else(|| v.to_string(), str::to_owned);
    Some(if items[2].is_nil() {
        Ok(text(&items[3]))
    } else {
        Err(text(&items[2]))
    })
}

// ---- Status reporter ----------------------------------------------------------------

/// Sends the server's status to the matrix once a second, plus a scrolling
/// message when the state or the board's IP address changes.
pub async fn report(state: AppState, matrix: Matrix) {
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_status = Status::Starting;
    let mut last_ip: Option<Option<String>> = None;

    for second in 0u64.. {
        tick.tick().await;
        let status = health(&state).await;
        if status != last_status {
            tracing::info!(?status, "server status changed");
            match status {
                Status::Warning => matrix.text("DISK LOW"),
                Status::Error => matrix.text("DB ERROR"),
                Status::Ok if last_status != Status::Starting => matrix.text("OK"),
                _ => {}
            }
        }
        // The board is headless: show where to reach it at start and on change.
        if second % 30 == 0 {
            let ip = tokio::task::spawn_blocking(primary_ipv4).await.ok().flatten();
            if last_ip.as_ref() != Some(&ip) {
                matrix.text(&ip.as_deref().map_or("NO NETWORK".to_owned(), |ip| format!("IP {ip}")));
                last_ip = Some(ip);
            }
        }
        let (requests, uploads) = state.metrics.take();
        matrix.status(status, requests, uploads);
        last_status = status;
    }
}

async fn health(state: &AppState) -> Status {
    let db = tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query("SELECT 1").execute(&state.db),
    )
    .await;
    if !matches!(db, Ok(Ok(_))) {
        return Status::Error;
    }
    match state.storage.available_bytes() {
        Ok(free) if free < state.settings.min_free_disk_bytes => Status::Warning,
        Ok(_) => Status::Ok,
        Err(_) => Status::Warning,
    }
}

/// The address of the interface that routes to the internet. UDP connect()
/// sends no packets; it only asks the kernel which source address it would use.
fn primary_ipv4() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:53").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified()).then(|| ip.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_all(mut bytes: &[u8]) -> Vec<Value> {
        let mut out = Vec::new();
        while !bytes.is_empty() {
            out.push(rmpv::decode::read_value(&mut bytes).unwrap());
        }
        out
    }

    #[test]
    fn rpc_messages() {
        let status = decode_all(&encode_msg(&Msg::Status(Status::Ok, 300, 3)));
        assert_eq!(status.len(), 1);
        assert_eq!(
            status[0],
            Value::Array(vec![
                2.into(),
                "notes/status".into(),
                Value::Array(vec!["O".into(), 300.into(), 3.into()]),
            ])
        );
        // Exactly the bytes the firmware's host test decodes.
        assert_eq!(
            encode_msg(&Msg::Status(Status::Ok, 300, 3)),
            b"\x93\x02\xacnotes/status\x93\xa1O\xcd\x01\x2c\x03"
        );
        assert_eq!(
            encode_msg(&Msg::Status(Status::Error, 123_456, 5000)),
            encode_msg(&Msg::Status(Status::Error, 9999, 999))
        );
        assert_eq!(
            decode_all(&request(7, "notes/hello", vec![]))[0],
            Value::Array(vec![0.into(), 7.into(), "notes/hello".into(), Value::Array(vec![])])
        );
    }

    #[test]
    fn responses() {
        let ok = Value::Array(vec![1.into(), 7.into(), Value::Nil, "notes-matrix 3".into()]);
        assert_eq!(parse_response(&ok), Some(Ok("notes-matrix 3".into())));
        let err = Value::Array(vec![
            1.into(),
            8.into(),
            Value::Array(vec![3.into(), "method notes/hello not available".into()]),
            Value::Nil,
        ]);
        assert!(matches!(parse_response(&err), Some(Err(e)) if e.contains("not available")));
        let notif = Value::Array(vec![2.into(), "x".into(), Value::Array(vec![])]);
        assert_eq!(parse_response(&notif), None);
    }

    #[test]
    fn text_sanitizing() {
        assert_eq!(sanitize_text("  ip 192.168.1.20 "), "IP 192.168.1.20");
        assert_eq!(sanitize_text("disk low ✗ <b>"), "DISK LOW B");
        assert_eq!(sanitize_text(&"x".repeat(100)).len(), MAX_TEXT_CHARS);
    }

    #[test]
    fn metrics_reset_on_take() {
        let m = Metrics::default();
        m.record_request();
        m.record_request();
        m.record_uploads(3);
        assert_eq!(m.take(), (2, 3));
        assert_eq!(m.take(), (0, 0));
    }

    #[test]
    fn link_writes_rpc_stream() {
        // On unix the link needs the router socket; elsewhere it appends to a file.
        if cfg!(unix) {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("router.bin");
        let matrix = Matrix::open(path.clone());
        matrix.status(Status::Ok, 4, 1);
        matrix.text("hello!");
        drop(matrix);
        let mut values = Vec::new();
        for _ in 0..100 {
            values = decode_all(&std::fs::read(&path).unwrap_or_default());
            if values.len() == 3 {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let methods: Vec<_> = values
            .iter()
            .map(|v| v.as_array().unwrap().iter().find_map(Value::as_str).unwrap().to_owned())
            .collect();
        assert_eq!(methods, ["notes/hello", "notes/status", "notes/text"]);
    }
}
