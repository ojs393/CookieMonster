use cookie_monster_core::store::EventStore;
use cookie_monster_core::types::*;
use rusqlite::Connection;
use tempfile::tempdir;

fn event() -> CookieEvent {
    CookieEvent {
        id: "event-1".into(),
        ts_ms: 1_700_000_000_000,
        action: CookieAction::Set,
        party: CookieParty::Third,
        scheme: "https".into(),
        host: "analytics.example".into(),
        name: "session".into(),
        value_redacted: true,
        attributes: CookieAttributes {
            secure: true,
            http_only: true,
            same_site: Some("Lax".into()),
            max_age: Some(3600),
            domain: Some(".example".into()),
            path: Some("/".into()),
        },
    }
}

#[test]
fn append_and_list_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("events.db");
    let key = [7u8; 32];
    let store = EventStore::open(&path, &key).unwrap();
    let event = event();
    let decision = Decision {
        kind: DecisionKind::Allow,
        reason: "observe".into(),
    };

    store
        .append(&event, &decision, CookieBucket::Unknown)
        .unwrap();
    let rows = store.list_recent(10).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].event, event);
    assert_eq!(rows[0].decision, decision);
    assert_eq!(rows[0].bucket, CookieBucket::Unknown);
    assert!(rows[0].stored_at_ms > 0);
}

#[test]
fn open_migrates_existing_databases_with_a_timestamp_index() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("events.db");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE events (
                id TEXT PRIMARY KEY,
                ts_ms INTEGER NOT NULL,
                nonce BLOB NOT NULL,
                ciphertext BLOB NOT NULL
            );",
        )
        .unwrap();
    drop(connection);

    let store = EventStore::open(&path, &[7u8; 32]).unwrap();
    drop(store);

    let connection = Connection::open(path).unwrap();
    let indexed_column: String = connection
        .query_row(
            "SELECT name FROM pragma_index_info('events_ts_ms_idx')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(indexed_column, "ts_ms");
}

#[test]
fn persists_only_encrypted_redacted_event_data() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("events.db");
    let store = EventStore::open(&path, &[7u8; 32]).unwrap();
    let mut event = event();
    event.value_redacted = false;
    let decision = Decision {
        kind: DecisionKind::Allow,
        reason: "sensitive-reason".into(),
    };

    store
        .append(&event, &decision, CookieBucket::Analytics)
        .unwrap();
    let rows = store.list_recent(1).unwrap();
    assert!(rows[0].event.value_redacted);
    drop(store);

    let connection = Connection::open(path).unwrap();
    let ciphertext: Vec<u8> = connection
        .query_row("SELECT ciphertext FROM events", [], |row| row.get(0))
        .unwrap();
    let ciphertext = String::from_utf8_lossy(&ciphertext);
    assert!(!ciphertext.contains("analytics.example"));
    assert!(!ciphertext.contains("session"));
    assert!(!ciphertext.contains("sensitive-reason"));
}

#[test]
fn prune_removes_only_rows_older_than_cutoff() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("events.db");
    let store = EventStore::open(&path, &[7u8; 32]).unwrap();
    let decision = Decision {
        kind: DecisionKind::Allow,
        reason: "observe".into(),
    };
    store
        .append(&event(), &decision, CookieBucket::Unknown)
        .unwrap();
    drop(store);

    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE events SET ts_ms = ts_ms - (31 * 24 * 60 * 60 * 1000)",
            [],
        )
        .unwrap();
    drop(connection);

    let store = EventStore::open(&path, &[7u8; 32]).unwrap();
    assert_eq!(store.prune_older_than_days(30).unwrap(), 1);
    assert!(store.list_recent(10).unwrap().is_empty());
}

#[test]
fn enforce_limits_prunes_aged_events() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("events.db");
    let store = EventStore::open(&path, &[7u8; 32]).unwrap();
    let decision = Decision {
        kind: DecisionKind::Allow,
        reason: "observe".into(),
    };
    store
        .append(&event(), &decision, CookieBucket::Unknown)
        .unwrap();
    drop(store);

    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE events SET ts_ms = ts_ms - (40 * 24 * 60 * 60 * 1000)",
            [],
        )
        .unwrap();
    drop(connection);

    let store = EventStore::open(&path, &[7u8; 32]).unwrap();
    assert_eq!(store.enforce_limits_with_retention(30).unwrap(), 1);
    assert_eq!(store.event_count().unwrap(), 0);
}

#[test]
fn enforce_limits_caps_to_configured_max() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("events.db");
    let store = EventStore::open(&path, &[9u8; 32]).unwrap();
    let decision = Decision {
        kind: DecisionKind::Allow,
        reason: "observe".into(),
    };
    for i in 0..8 {
        let mut e = event();
        e.id = format!("cap-{i}");
        e.ts_ms = 1_700_000_000_000 + i;
        store
            .append(&e, &decision, CookieBucket::Unknown)
            .unwrap();
    }
    assert_eq!(store.event_count().unwrap(), 8);
    let deleted = store.enforce_limits_configured(365, 5).unwrap();
    assert_eq!(deleted, 3);
    assert_eq!(store.event_count().unwrap(), 5);
    let remaining = store.list_recent(10).unwrap();
    assert_eq!(remaining.len(), 5);
    assert!(remaining.iter().all(|r| r.event.id.starts_with("cap-")));
}

#[test]
fn wrong_key_cannot_decrypt_rows() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("events.db");
    let store = EventStore::open(&path, &[7u8; 32]).unwrap();
    store
        .append(
            &event(),
            &Decision {
                kind: DecisionKind::Allow,
                reason: "observe".into(),
            },
            CookieBucket::Unknown,
        )
        .unwrap();
    drop(store);

    let store = EventStore::open(&path, &[8u8; 32]).unwrap();
    assert!(store.list_recent(1).is_err());
}
