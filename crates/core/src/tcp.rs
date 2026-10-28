
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rand::rngs::OsRng;
use rand::RngCore;
use serde_json::{json, Value};

use crate::engine::Engine;
use crate::ipc::handle_ipc_value;

pub const DEFAULT_TCP_ADDR: &str = "127.0.0.1:9735";
pub const TOKEN_FILE_NAME: &str = "ipc.token";
pub const MAX_LINE_BYTES: usize = 1024 * 1024;
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

pub type SharedEngine = Arc<Mutex<Engine>>;

pub fn write_fresh_token(path: &Path) -> io::Result<String> {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();

    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "token path has no parent"))?;
    let temp = parent.join(format!(".ipc.token.tmp-{}", std::process::id()));
    let _ = fs::remove_file(&temp);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temp)?;
    file.write_all(token.as_bytes())?;
    file.sync_all()?;
    drop(file);
    #[cfg(unix)]
    fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
    fs::rename(&temp, path)?;
    Ok(token)
}

fn tokens_match(expected: &str, provided: &str) -> bool {
    let (a, b) = (expected.as_bytes(), provided.as_bytes());
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len() {
        diff |= (a[i] ^ b.get(i).copied().unwrap_or(0)) as usize;
    }
    diff == 0
}

fn read_bounded_line(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    let read = reader
        .by_ref()
        .take(MAX_LINE_BYTES as u64 + 1)
        .read_until(b'\n', &mut line)?;
    if read == 0 {
        return Ok(None);
    }
    if line.last() == Some(&b'\n') {
        line.pop();
        return Ok(Some(line));
    }
    if line.len() > MAX_LINE_BYTES {
        return Err(io::Error::new(ErrorKind::InvalidData, "request line too long"));
    }
    Ok(Some(line))
}

fn write_line(stream: &mut TcpStream, value: &Value) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(value).expect("JSON values serialize");
    bytes.push(b'\n');
    stream.write_all(&bytes)?;
    stream.flush()
}

fn error_value(message: &str) -> Value {
    json!({"type": "error", "message": message})
}

fn handle_connection(mut stream: TcpStream, engine: SharedEngine, token: Arc<str>) {
    let _ = stream.set_read_timeout(Some(IDLE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IDLE_TIMEOUT));
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);

    loop {
        let line = match read_bounded_line(&mut reader) {
            Ok(Some(line)) => line,
            Ok(None) | Err(_) => return,
        };
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }

        let mut value: Value = match serde_json::from_slice(&line) {
            Ok(value) => value,
            Err(error) => {
                if write_line(&mut stream, &error_value(&format!("invalid JSON: {error}"))).is_err()
                {
                    return;
                }
                continue;
            }
        };

        let authorized = value
            .get("token")
            .and_then(Value::as_str)
            .is_some_and(|provided| tokens_match(&token, provided));
        if !authorized {
            let _ = write_line(&mut stream, &error_value("unauthorized"));
            return;
        }
        if let Some(object) = value.as_object_mut() {
            object.remove("token");
        }

        let response = {
            let mut engine = engine.lock().unwrap_or_else(|e| e.into_inner());
            handle_ipc_value(&mut engine, value)
        };
        if write_line(&mut stream, &response).is_err() {
            return;
        }
    }
}

pub fn serve(listener: TcpListener, engine: SharedEngine, token: String) {
    let token: Arc<str> = Arc::from(token);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let engine = Arc::clone(&engine);
                let token = Arc::clone(&token);
                thread::spawn(move || handle_connection(stream, engine, token));
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    }
}

pub fn spawn(listener: TcpListener, engine: SharedEngine, token: String) -> JoinHandle<()> {
    thread::spawn(move || serve(listener, engine, token))
}
