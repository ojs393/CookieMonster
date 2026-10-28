use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use cookie_monster_core::engine::Engine;
use cookie_monster_core::store::EventStore;
use cookie_monster_core::tcp::{spawn, write_fresh_token, TOKEN_FILE_NAME};
use cookie_monster_core::types::Mode;
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};

const TOKEN: &str = "test-token";

fn start() -> (TempDir, std::net::SocketAddr) {
    let dir = tempdir().unwrap();
    let store = EventStore::open(&dir.path().join("events.db"), &[7u8; 32]).unwrap();
    let engine = Arc::new(Mutex::new(Engine::new(Mode::Observe, store)));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    spawn(listener, engine, TOKEN.to_string());
    (dir, addr)
}

struct Client {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl Client {
    fn connect(addr: std::net::SocketAddr) -> Self {
        let writer = TcpStream::connect(addr).unwrap();
        let reader = BufReader::new(writer.try_clone().unwrap());
        Self { reader, writer }
    }

    fn send_raw(&mut self, line: &str) -> Option<Value> {
        if self.writer.write_all(line.as_bytes()).is_err()
            || self.writer.write_all(b"\n").is_err()
        {
            return None;
        }
        let mut response = String::new();
        let read = self.reader.read_line(&mut response).unwrap_or(0);
        (read > 0).then(|| serde_json::from_str(&response).unwrap())
    }

    fn send(&mut self, value: Value) -> Option<Value> {
        self.send_raw(&value.to_string())
    }
}

#[test]
fn tcp_get_status_returns_observe() {
    let (_dir, addr) = start();
    let mut client = Client::connect(addr);

    let response = client
        .send(json!({"token": TOKEN, "type": "get_status"}))
        .unwrap();

    assert_eq!(
        response,
        json!({
            "type": "status",
            "mode": "observe",
            "browser_shield": true,
            "system_shield": false
        })
    );
}

#[test]
fn tcp_rejects_missing_or_wrong_token_and_closes() {
    let (_dir, addr) = start();

    for request in [
        json!({"type": "get_status"}),
        json!({"token": "wrong", "type": "get_status"}),
        json!({"token": 42, "type": "get_status"}),
    ] {
        let mut client = Client::connect(addr);
        let response = client.send(request).unwrap();
        assert_eq!(response["type"], "error");
        assert_eq!(response["message"], "unauthorized");
        assert!(client
            .send(json!({"token": TOKEN, "type": "get_status"}))
            .is_none());
    }
}

#[test]
fn tcp_serves_multiple_requests_and_shares_state_across_connections() {
    let (_dir, addr) = start();
    let event = json!({
        "id": "tcp-event-1",
        "ts_ms": 1_700_000_000_000_u64,
        "action": "Set",
        "party": "Third",
        "scheme": "https",
        "host": "tracker.example",
        "name": "_ga",
        "value_redacted": true,
        "attributes": {
            "secure": true, "http_only": false, "same_site": null,
            "max_age": null, "domain": null, "path": "/"
        }
    });

    let mut first = Client::connect(addr);
    let decision = first
        .send(json!({"token": TOKEN, "type": "cookie_event", "event": event}))
        .unwrap();
    assert_eq!(decision["type"], "decision");
    let status = first
        .send(json!({"token": TOKEN, "type": "set_mode", "mode": "moderate"}))
        .unwrap();
    assert_eq!(status["mode"], "moderate");

    let mut second = Client::connect(addr);
    let status = second
        .send(json!({"token": TOKEN, "type": "get_status"}))
        .unwrap();
    assert_eq!(status["mode"], "moderate");
    let events = second
        .send(json!({"token": TOKEN, "type": "list_events", "limit": 10}))
        .unwrap();
    assert_eq!(events["type"], "events");
    assert_eq!(events["items"][0]["event"]["host"], "tracker.example");
}

#[test]
fn tcp_reports_invalid_json_and_keeps_connection_open() {
    let (_dir, addr) = start();
    let mut client = Client::connect(addr);

    let response = client.send_raw("{not json").unwrap();
    assert_eq!(response["type"], "error");

    let response = client
        .send(json!({"token": TOKEN, "type": "get_status"}))
        .unwrap();
    assert_eq!(response["type"], "status");
}

#[test]
fn tcp_closes_connection_on_oversized_line() {
    let (_dir, addr) = start();
    let mut client = Client::connect(addr);
    let huge = "x".repeat(1024 * 1024 + 16);
    let _ = client.writer.write_all(huge.as_bytes());
    let _ = client.writer.write_all(b"\n");
    let mut response = String::new();
    let read = client.reader.read_line(&mut response).unwrap_or(0);
    assert_eq!(read, 0);
}

#[test]
fn token_file_is_fresh_owner_only_and_rotates() {
    let dir = tempdir().unwrap();
    let path = dir.path().join(TOKEN_FILE_NAME);

    let first = write_fresh_token(&path).unwrap();
    assert_eq!(first.len(), 64);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    let second = write_fresh_token(&path).unwrap();
    assert_ne!(first, second);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), second);
}
