
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

pub const DEFAULT_ADDR: &str = "127.0.0.1:9735";
pub const EVENT_LIMIT: usize = 500;
const TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FeedEvent {
    pub host: String,
    pub name: String,
    pub action: String,
    pub decision: String,
    pub bucket: String,
    pub stored_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Snapshot {
    pub connected: bool,
    pub mode: Option<String>,
    pub browser_shield: bool,
    pub system_shield: bool,
    pub events: Vec<FeedEvent>,
    pub error: Option<String>,
}

impl Snapshot {
    pub fn disconnected(error: impl Into<String>) -> Self {
        Self {
            connected: false,
            mode: None,
            browser_shield: false,
            system_shield: false,
            events: Vec::new(),
            error: Some(error.into()),
        }
    }
}

pub fn token_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("com", "CookieMonster", "cookie-monster")
        .map(|dirs| dirs.data_dir().join("ipc.token"))
}

pub fn listen_addr() -> String {
    std::env::var("COOKIE_MONSTER_TCP_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.into())
}

fn request(
    reader: &mut BufReader<TcpStream>,
    writer: &mut TcpStream,
    token: &str,
    mut body: Value,
) -> Result<Value, String> {
    body["token"] = Value::String(token.to_string());
    let mut line = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
    line.push(b'\n');
    writer.write_all(&line).map_err(|e| e.to_string())?;
    let mut response = String::new();
    let read = reader.read_line(&mut response).map_err(|e| e.to_string())?;
    if read == 0 {
        return Err("host closed the connection".into());
    }
    let value: Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    if value["type"] == "error" {
        return Err(value["message"].as_str().unwrap_or("host error").to_string());
    }
    Ok(value)
}

pub fn fetch_snapshot(addr: &str, token_file: &Path) -> Snapshot {
    let token = match std::fs::read_to_string(token_file) {
        Ok(token) => token.trim().to_string(),
        Err(_) => return Snapshot::disconnected("host not running (no ipc.token)"),
    };
    let Some(socket) = addr.to_socket_addrs().ok().and_then(|mut a| a.next()) else {
        return Snapshot::disconnected(format!("bad address {addr}"));
    };
    let stream = match TcpStream::connect_timeout(&socket, TIMEOUT) {
        Ok(stream) => stream,
        Err(error) => return Snapshot::disconnected(format!("host unreachable: {error}")),
    };
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    let _ = stream.set_write_timeout(Some(TIMEOUT));
    let Ok(mut writer) = stream.try_clone() else {
        return Snapshot::disconnected("socket clone failed");
    };
    let mut reader = BufReader::new(stream);

    let status = match request(&mut reader, &mut writer, &token, json!({"type": "get_status"})) {
        Ok(status) => status,
        Err(error) => return Snapshot::disconnected(error),
    };
    let events = match request(
        &mut reader,
        &mut writer,
        &token,
        json!({"type": "list_events", "limit": EVENT_LIMIT}),
    ) {
        Ok(events) => events,
        Err(error) => return Snapshot::disconnected(error),
    };

    Snapshot {
        connected: true,
        mode: status["mode"].as_str().map(str::to_string),
        browser_shield: status["browser_shield"].as_bool().unwrap_or(true),
        system_shield: status["system_shield"].as_bool().unwrap_or(false),
        events: parse_events(&events),
        error: None,
    }
}

fn parse_events(response: &Value) -> Vec<FeedEvent> {
    let text = |value: &Value| value.as_str().unwrap_or_default().to_string();
    response["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| FeedEvent {
                    host: text(&item["event"]["host"]),
                    name: text(&item["event"]["name"]),
                    action: text(&item["event"]["action"]),
                    decision: text(&item["decision"]["kind"]),
                    bucket: text(&item["bucket"]),
                    stored_at_ms: item["stored_at_ms"].as_i64().unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn mode_label(mode: &str) -> String {
    match mode {
        "observe" => "Observe".into(),
        "moderate" => "Moderate".into(),
        "cookie_monster" => "Cookie Monster".into(),
        other => other.into(),
    }
}

pub fn rpc(addr: &str, token_file: &Path, body: Value) -> Result<Value, String> {
    let token = std::fs::read_to_string(token_file)
        .map_err(|_| "host not running (no ipc.token)".to_string())?;
    let token = token.trim();
    let socket = addr
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .ok_or_else(|| format!("bad address {addr}"))?;
    let stream = TcpStream::connect_timeout(&socket, TIMEOUT)
        .map_err(|error| format!("host unreachable: {error}"))?;
    let _ = stream.set_read_timeout(Some(TIMEOUT));
    let _ = stream.set_write_timeout(Some(TIMEOUT));
    let mut writer = stream
        .try_clone()
        .map_err(|_| "socket clone failed".to_string())?;
    let mut reader = BufReader::new(stream);
    request(&mut reader, &mut writer, token, body)
}

pub fn set_mode(addr: &str, token_file: &Path, mode: &str) -> Result<String, String> {
    let response = rpc(
        addr,
        token_file,
        json!({"type": "set_mode", "mode": mode}),
    )?;
    response["mode"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "missing mode in status response".into())
}

pub fn set_allowlist_hosts(addr: &str, token_file: &Path, hosts: &[String]) -> Result<(), String> {
    let entries: Vec<Value> = hosts
        .iter()
        .filter(|h| !h.trim().is_empty())
        .map(|host| json!({"host": host.trim(), "allowed_buckets": null}))
        .collect();
    let response = rpc(
        addr,
        token_file,
        json!({"type": "set_allowlist", "entries": entries}),
    )?;
    if response["type"] == "allowlist" {
        Ok(())
    } else {
        Err(format!("unexpected response: {response}"))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StoreStats {
    pub event_count: u64,
    pub retention_days: u32,
}

pub fn clear_events(addr: &str, token_file: &Path) -> Result<u64, String> {
    let response = rpc(addr, token_file, json!({"type": "clear_events"}))?;
    Ok(response["deleted"].as_u64().unwrap_or(0))
}

pub fn get_store_stats(addr: &str, token_file: &Path) -> Result<StoreStats, String> {
    let response = rpc(addr, token_file, json!({"type": "get_store_stats"}))?;
    Ok(StoreStats {
        event_count: response["event_count"].as_u64().unwrap_or(0),
        retention_days: response["retention_days"].as_u64().unwrap_or(30) as u32,
    })
}

pub fn set_retention_days(addr: &str, token_file: &Path, days: u32) -> Result<StoreStats, String> {
    let response = rpc(
        addr,
        token_file,
        json!({"type": "set_retention_days", "days": days}),
    )?;
    if response["type"] == "error" {
        return Err(response["message"]
            .as_str()
            .unwrap_or("set_retention_days failed")
            .to_string());
    }
    Ok(StoreStats {
        event_count: response["event_count"].as_u64().unwrap_or(0),
        retention_days: response["retention_days"].as_u64().unwrap_or(days as u64) as u32,
    })
}

pub fn set_system_shield(addr: &str, token_file: &Path, enabled: bool) -> Result<bool, String> {
    let response = rpc(
        addr,
        token_file,
        json!({"type": "set_system_shield", "enabled": enabled}),
    )?;
    Ok(response["system_shield"].as_bool().unwrap_or(enabled))
}

pub fn get_allowlist(addr: &str, token_file: &Path) -> Result<Vec<String>, String> {
    let response = rpc(addr, token_file, json!({"type": "get_allowlist"}))?;
    let hosts = response["entries"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|e| e["host"].as_str().map(str::to_string))
        .collect();
    Ok(hosts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    fn fake_host(token: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut writer = stream.try_clone().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                let req: Value = serde_json::from_str(&line).unwrap();
                line.clear();
                let reply = if req["token"] != token {
                    json!({"type": "error", "message": "unauthorized"})
                } else if req["type"] == "get_status" {
                    json!({"type": "status", "mode": "observe", "browser_shield": true, "system_shield": false})
                } else {
                    json!({"type": "events", "items": [{
                        "event": {"host": "ads.example", "name": "_ga", "action": "Set"},
                        "decision": {"kind": "Allow", "reason": "observe"},
                        "bucket": "Analytics",
                        "stored_at_ms": 5
                    }]})
                };
                writeln!(writer, "{reply}").unwrap();
            }
        });
        addr
    }

    #[test]
    fn snapshot_reports_observe_and_events() {
        let addr = fake_host("tok");
        let dir = std::env::temp_dir().join(format!("cm-desktop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let token_file = dir.join("ipc.token");
        std::fs::write(&token_file, "tok\n").unwrap();

        let snapshot = fetch_snapshot(&addr, &token_file);
        assert!(snapshot.connected, "{:?}", snapshot.error);
        assert_eq!(snapshot.mode.as_deref(), Some("observe"));
        assert_eq!(snapshot.events.len(), 1);
        assert_eq!(snapshot.events[0].host, "ads.example");
        assert_eq!(snapshot.events[0].decision, "Allow");
        assert_eq!(mode_label("observe"), "Observe");
    }

    #[test]
    fn missing_token_file_is_disconnected() {
        let snapshot = fetch_snapshot("127.0.0.1:1", Path::new("/nonexistent/ipc.token"));
        assert!(!snapshot.connected);
        assert!(snapshot.error.is_some());
    }

    fn policy_host(token: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut writer = stream.try_clone().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                let req: Value = serde_json::from_str(&line).unwrap();
                line.clear();
                let reply = if req["token"] != token {
                    json!({"type": "error", "message": "unauthorized"})
                } else if req["type"] == "set_mode" {
                    json!({
                        "type": "status",
                        "mode": req["mode"],
                        "browser_shield": true,
                        "system_shield": false
                    })
                } else if req["type"] == "set_allowlist" {
                    json!({"type": "allowlist", "entries": req["entries"]})
                } else {
                    json!({"type": "error", "message": format!("unexpected {}", req["type"])})
                };
                writeln!(writer, "{reply}").unwrap();
            }
        });
        addr
    }

    #[test]
    fn set_mode_returns_new_mode_from_status() {
        let addr = policy_host("tok");
        let dir = std::env::temp_dir().join(format!("cm-desktop-mode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let token_file = dir.join("ipc.token");
        std::fs::write(&token_file, "tok\n").unwrap();

        let mode = set_mode(&addr, &token_file, "cookie_monster").unwrap();
        assert_eq!(mode, "cookie_monster");
        assert_eq!(mode_label("cookie_monster"), "Cookie Monster");
    }

    #[test]
    fn set_allowlist_hosts_sends_entries() {
        let addr = policy_host("tok");
        let dir = std::env::temp_dir().join(format!("cm-desktop-al-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let token_file = dir.join("ipc.token");
        std::fs::write(&token_file, "tok\n").unwrap();

        set_allowlist_hosts(
            &addr,
            &token_file,
            &["github.com".into(), "example.com".into()],
        )
        .unwrap();
    }
}
