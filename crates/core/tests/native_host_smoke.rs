use cookie_monster_core::engine::Engine;
use cookie_monster_core::ipc::handle_ipc_value;
use cookie_monster_core::store::{load_or_create_key, EventStore};
use cookie_monster_core::types::Mode;
use serde_json::{json, Value};
use tempfile::tempdir;

fn dispatch(engine: &mut Engine, request: Value) -> Value {
    handle_ipc_value(engine, request)
}

#[test]
fn dispatches_native_host_requests_and_reports_invalid_messages() {
    let dir = tempdir().unwrap();
    let store = EventStore::open(&dir.path().join("events.db"), &[7u8; 32]).unwrap();
    let mut engine = Engine::new(Mode::Observe, store);

    assert_eq!(
        dispatch(&mut engine, json!({"type": "get_status"})),
        json!({
            "type": "status",
            "mode": "observe",
            "browser_shield": true,
            "system_shield": false
        })
    );

    assert_eq!(
        dispatch(&mut engine, json!({"type": "set_mode", "mode": "moderate"})),
        json!({
            "type": "status",
            "mode": "moderate",
            "browser_shield": true,
            "system_shield": false
        })
    );

    let tracker = json!({
        "id": "native-host-event-moderate",
        "ts_ms": 1_700_000_000_000_u64,
        "action": "Set",
        "party": "Third",
        "scheme": "https",
        "host": "analytics.example",
        "name": "_ga",
        "value_redacted": true,
        "attributes": {
            "secure": true,
            "http_only": false,
            "same_site": null,
            "max_age": null,
            "domain": null,
            "path": "/"
        }
    });
    let response = dispatch(
        &mut engine,
        json!({"type": "cookie_event", "event": tracker}),
    );
    assert_eq!(response["type"], "decision");
    assert_eq!(response["decision"]["kind"], "Deny");

    assert_eq!(
        dispatch(&mut engine, json!({"type": "set_mode", "mode": "observe"})),
        json!({
            "type": "status",
            "mode": "observe",
            "browser_shield": true,
            "system_shield": false
        })
    );

    let event = json!({
        "id": "native-host-event-1",
        "ts_ms": 1_700_000_000_001_u64,
        "action": "Set",
        "party": "Third",
        "scheme": "https",
        "host": "analytics.example",
        "name": "_ga",
        "value_redacted": true,
        "attributes": {
            "secure": true,
            "http_only": false,
            "same_site": null,
            "max_age": null,
            "domain": null,
            "path": "/"
        }
    });
    let response = dispatch(&mut engine, json!({"type": "cookie_event", "event": event}));
    assert_eq!(response["type"], "decision");
    assert_eq!(response["decision"]["kind"], "Allow");

    let response = dispatch(&mut engine, json!({"type": "list_events", "limit": 10}));
    assert_eq!(response["type"], "events");
    let items = response["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|item| item["event"]["id"] == "native-host-event-1"));
    assert!(items.iter().any(|item| {
        item["event"]["id"] == "native-host-event-moderate"
            && item["decision"]["kind"] == "Deny"
    }));

    let response = dispatch(&mut engine, json!({"type": "unknown"}));
    assert_eq!(response["type"], "error");
    assert!(response["message"].as_str().unwrap().contains("unknown"));
}

#[test]
fn store_failure_returns_an_ipc_error_response() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("events.db");
    let store = EventStore::open(&path, &[7u8; 32]).unwrap();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER reject_event_insert
             BEFORE INSERT ON events
             BEGIN
                 SELECT RAISE(FAIL, 'simulated store failure');
             END;",
        )
        .unwrap();
    let mut engine = Engine::new(Mode::Observe, store);
    let event = json!({
        "id": "failing-event",
        "ts_ms": 1_700_000_000_000_u64,
        "action": "Set",
        "party": "Third",
        "scheme": "https",
        "host": "analytics.example",
        "name": "_ga",
        "value_redacted": true,
        "attributes": {
            "secure": true,
            "http_only": false,
            "same_site": null,
            "max_age": null,
            "domain": null,
            "path": "/"
        }
    });

    let response = dispatch(&mut engine, json!({"type": "cookie_event", "event": event}));

    assert_eq!(response["type"], "error");
    assert!(response["message"]
        .as_str()
        .unwrap()
        .contains("simulated store failure"));
}

#[test]
fn creates_and_reuses_a_private_store_key() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("store.key");

    let created = load_or_create_key(&path).unwrap();
    let loaded = load_or_create_key(&path).unwrap();

    assert_eq!(created, loaded);
    assert_eq!(std::fs::read(&path).unwrap(), created);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn concurrent_key_initializers_all_receive_the_published_key() {
    use std::sync::{Arc, Barrier};

    const WORKERS: usize = 32;
    let dir = tempdir().unwrap();
    let path = Arc::new(dir.path().join("store.key"));
    let barrier = Arc::new(Barrier::new(WORKERS));
    let handles: Vec<_> = (0..WORKERS)
        .map(|_| {
            let path = Arc::clone(&path);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                load_or_create_key(&path)
            })
        })
        .collect();

    let keys: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap().unwrap())
        .collect();

    assert!(keys.iter().all(|key| key == &keys[0]));
    assert_eq!(std::fs::read(path.as_ref()).unwrap(), keys[0]);
}

#[test]
fn list_events_clamps_untrusted_limits() {
    let dir = tempdir().unwrap();
    let store = EventStore::open(&dir.path().join("events.db"), &[7u8; 32]).unwrap();
    let mut engine = Engine::new(Mode::Observe, store);

    for id in 0..=1000 {
        dispatch(
            &mut engine,
            json!({
                "type": "cookie_event",
                "event": {
                    "id": format!("event-{id}"),
                    "ts_ms": 1_700_000_000_000_u64 + id,
                    "action": "Set",
                    "party": "Third",
                    "scheme": "https",
                    "host": "analytics.example",
                    "name": "_ga",
                    "value_redacted": true,
                    "attributes": {
                        "secure": true,
                        "http_only": false,
                        "same_site": null,
                        "max_age": null,
                        "domain": null,
                        "path": "/"
                    }
                }
            }),
        );
    }

    let response = dispatch(
        &mut engine,
        json!({"type": "list_events", "limit": usize::MAX}),
    );

    assert_eq!(response["items"].as_array().unwrap().len(), 1000);
}
