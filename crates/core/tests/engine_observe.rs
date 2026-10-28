use cookie_monster_core::engine::Engine;
use cookie_monster_core::store::EventStore;
use cookie_monster_core::types::{CookieAction, CookieEvent, CookieParty, DecisionKind, Mode};
use tempfile::tempdir;

fn event() -> CookieEvent {
    CookieEvent {
        id: "engine-event-1".into(),
        ts_ms: 1_700_000_000_000,
        action: CookieAction::Set,
        party: CookieParty::Third,
        scheme: "https".into(),
        host: "analytics.example".into(),
        name: "_ga".into(),
        value_redacted: true,
        attributes: Default::default(),
    }
}

#[test]
fn observe_allows_and_persists_the_classified_event() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("events.db");
    let key = [7u8; 32];
    let store = EventStore::open(&path, &key).unwrap();
    let mut engine = Engine::new(Mode::Observe, store);

    let decision = engine.handle_event(event()).unwrap();

    assert_eq!(decision.kind, DecisionKind::Allow);
    let rows = EventStore::open(&path, &key)
        .unwrap()
        .list_recent(10)
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].event.id, "engine-event-1");
}

#[test]
fn duplicate_event_is_idempotent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("events.db");
    let key = [7u8; 32];
    let store = EventStore::open(&path, &key).unwrap();
    let mut engine = Engine::new(Mode::Observe, store);

    let first = engine.handle_event(event()).unwrap();
    let duplicate = engine.handle_event(event()).unwrap();

    assert_eq!(duplicate, first);
    let rows = EventStore::open(&path, &key)
        .unwrap()
        .list_recent(10)
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[test]
fn mode_can_be_changed_and_read() {
    let dir = tempdir().unwrap();
    let store = EventStore::open(&dir.path().join("events.db"), &[7u8; 32]).unwrap();
    let mut engine = Engine::new(Mode::Observe, store);

    engine.set_mode(Mode::CookieMonster).unwrap();

    assert_eq!(engine.mode(), Mode::CookieMonster);
}

#[test]
fn cookie_monster_mode_denies_sets() {
    let dir = tempdir().unwrap();
    let store = EventStore::open(&dir.path().join("events.db"), &[7u8; 32]).unwrap();
    let mut engine = Engine::new(Mode::CookieMonster, store);

    let decision = engine.handle_event(event()).unwrap();

    assert_eq!(decision.kind, DecisionKind::Deny);
    assert_eq!(decision.reason, "cookie_monster");
}

#[test]
fn store_failure_is_returned_instead_of_panicking() {
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

    let error = engine.handle_event(event()).unwrap_err();

    assert!(error.to_string().contains("simulated store failure"));
}
