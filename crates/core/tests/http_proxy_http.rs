
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cookie_monster_core::engine::Engine;
use cookie_monster_core::http_proxy;
use cookie_monster_core::policy_state::{self, PolicyState};
use cookie_monster_core::store::{load_or_create_key, EventStore};
use cookie_monster_core::types::Mode;
use tempfile::tempdir;

fn start_upstream() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        let body = b"ok";
        let resp = format!(
            "HTTP/1.1 200 OK\r\nSet-Cookie: tracker=1; Path=/\r\nContent-Length: {}\r\nConnection: close\r\n\r\nok",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes());
    });
    (addr, handle)
}

#[test]
fn proxy_logs_set_cookie_on_http() {
    let dir = tempdir().unwrap();
    let key = load_or_create_key(&dir.path().join("store.key")).unwrap();
    let store = EventStore::open(&dir.path().join("events.db"), &key).unwrap();
    let policy_path = dir.path().join("policy.json");
    let mut state = PolicyState::default();
    state.mode = Mode::Observe;
    state.system_shield = true;
    policy_state::save_policy_state(&policy_path, &state).unwrap();
    let engine = Arc::new(Mutex::new(Engine::from_policy_state(
        store,
        state,
        policy_path,
    )));

    let proxy_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap().to_string();
    drop(proxy_listener);

    let handle = http_proxy::spawn(&proxy_addr, Arc::clone(&engine), None, true).unwrap();
    assert!(handle.is_enabled());

    let (upstream, upstream_thread) = start_upstream();
    thread::sleep(Duration::from_millis(50));

    let mut client = TcpStream::connect(&proxy_addr).unwrap();
    let req = format!(
        "GET http://{upstream}/path HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        upstream.split(':').next().unwrap()
    );
    client.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    client.read_to_string(&mut resp).unwrap();
    assert!(resp.contains("200"), "{resp}");
    assert!(resp.to_lowercase().contains("set-cookie"), "{resp}");

    thread::sleep(Duration::from_millis(100));
    let events = engine.lock().unwrap().list_events(10).unwrap();
    assert!(
        events.iter().any(|e| e.event.name == "tracker"),
        "expected tracker cookie event, got {events:?}"
    );
    let _ = upstream_thread.join();
}
