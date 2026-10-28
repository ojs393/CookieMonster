use std::error::Error;
use std::io::{self, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use cookie_monster_core::ca::LocalCa;
use cookie_monster_core::engine::Engine;
use cookie_monster_core::http_proxy::{self, DEFAULT_PROXY_ADDR};
use cookie_monster_core::ipc::{
    decode_native_message, encode_native_message, handle_ipc_value, IpcError,
};
use cookie_monster_core::policy_state::{self, POLICY_FILE_NAME};
use cookie_monster_core::store::{load_or_create_key, EventStore};
use cookie_monster_core::tcp::{self, DEFAULT_TCP_ADDR, TOKEN_FILE_NAME};
use directories::ProjectDirs;
use serde_json::Value;

fn main() -> Result<(), Box<dyn Error>> {
    let tcp_only = std::env::args().any(|arg| arg == "--tcp-only");

    let project_dirs = ProjectDirs::from("com", "CookieMonster", "cookie-monster")
        .ok_or("could not determine the CookieMonster data directory")?;
    let data_dir = project_dirs.data_dir();
    std::fs::create_dir_all(data_dir)?;

    let key = load_or_create_key(&data_dir.join("store.key"))?;
    let store = EventStore::open(&data_dir.join("events.db"), &key)?;
    let policy_path = data_dir.join(POLICY_FILE_NAME);
    let policy = policy_state::load_policy_state(&policy_path)?;
    let system_shield_on = policy.system_shield;
    let engine = Arc::new(Mutex::new(Engine::from_policy_state(
        store,
        policy,
        policy_path,
    )));
    if let Ok(eng) = engine.lock() {
        let _ = eng.enforce_store_limits();
    }

    {
        let eng = Arc::clone(&engine);
        let log_path = data_dir.join("native-host.log");
        thread::spawn(move || loop {
            thread::sleep(std::time::Duration::from_secs(3600));
            if let Ok(eng) = eng.lock() {
                let _ = eng.enforce_store_limits();
            }
            if let Ok(meta) = std::fs::metadata(&log_path) {
                if meta.len() > 2 * 1024 * 1024 {
                    let bak = log_path.with_extension("log.1");
                    let _ = std::fs::remove_file(&bak);
                    let _ = std::fs::rename(&log_path, &bak);
                }
            }
        });
    }

    let ca = match LocalCa::load_or_create(data_dir) {
        Ok(ca) => {
            eprintln!("cookie-monster: local CA ready");
            Some(Arc::new(ca))
        }
        Err(error) => {
            eprintln!("cookie-monster: local CA unavailable (HTTPS MITM off): {error}");
            None
        }
    };

    let proxy_addr =
        std::env::var("COOKIE_MONSTER_PROXY_ADDR").unwrap_or_else(|_| DEFAULT_PROXY_ADDR.into());
    let proxy = match http_proxy::spawn(
        &proxy_addr,
        Arc::clone(&engine),
        ca.clone(),
        system_shield_on,
    ) {
        Ok(handle) => {
            let mitm = if ca.is_some() { "MITM on" } else { "HTTP only" };
            eprintln!(
                "cookie-monster: system shield proxy on {proxy_addr} (enabled={system_shield_on}, {mitm})"
            );
            Some(handle)
        }
        Err(error) => {
            eprintln!("cookie-monster: system shield proxy unavailable: {error}");
            None
        }
    };

    let addr = std::env::var("COOKIE_MONSTER_TCP_ADDR").unwrap_or_else(|_| DEFAULT_TCP_ADDR.into());
    let tcp_thread = match TcpListener::bind(&addr) {
        Ok(listener) => {
            let token = tcp::write_fresh_token(&data_dir.join(TOKEN_FILE_NAME))?;
            eprintln!("cookie-monster: serving UI IPC on {addr}");
            Some(tcp::spawn(listener, Arc::clone(&engine), token))
        }
        Err(error) if tcp_only => return Err(format!("cannot bind {addr}: {error}").into()),
        Err(error) => {
            eprintln!("cookie-monster: UI IPC disabled, cannot bind {addr}: {error}");
            None
        }
    };

    if let Some(proxy) = proxy.clone() {
        let eng = Arc::clone(&engine);
        thread::spawn(move || loop {
            let on = eng
                .lock()
                .map(|e| e.system_shield())
                .unwrap_or(false);
            proxy.set_enabled(on);
            thread::sleep(std::time::Duration::from_millis(500));
        });
    }

    if tcp_only {
        if let Some(handle) = tcp_thread {
            let _ = handle.join();
        } else {
            loop {
                thread::park();
            }
        }
        return Ok(());
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();

    loop {
        let value = match decode_native_message(&mut input) {
            Ok(value) => value,
            Err(IpcError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        };
        let response: Value = {
            let mut engine = engine.lock().unwrap_or_else(|e| e.into_inner());
            handle_ipc_value(&mut engine, value)
        };
        if let Some(proxy) = &proxy {
            if let Some(on) = response.get("system_shield").and_then(|v| v.as_bool()) {
                proxy.set_enabled(on);
            }
        }
        output.write_all(&encode_native_message(&response)?)?;
        output.flush()?;
    }

    Ok(())
}
