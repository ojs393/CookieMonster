use cookie_monster_core::engine::Engine;
use cookie_monster_core::ipc::handle_ipc_value;
use cookie_monster_core::policy_state::PolicyState;
use cookie_monster_core::store::{load_or_create_key, EventStore};
use cookie_monster_core::types::{
    CookieAction, CookieAttributes, CookieEvent, CookieParty, Mode,
};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn clear_events_empties_store_via_ipc() {
    let dir = tempdir().unwrap();
    let key = load_or_create_key(&dir.path().join("store.key")).unwrap();
    let store = EventStore::open(&dir.path().join("events.db"), &key).unwrap();
    let policy_path = dir.path().join("policy.json");
    let mut state = PolicyState::default();
    state.mode = Mode::Observe;
    let mut engine = Engine::from_policy_state(store, state, policy_path);

    let event = CookieEvent {
        id: "e1".into(),
        ts_ms: 1,
        action: CookieAction::Set,
        party: CookieParty::First,
        scheme: "https".into(),
        host: "example.com".into(),
        name: "sid".into(),
        value_redacted: true,
        attributes: CookieAttributes::default(),
    };
    engine.handle_event(event).unwrap();
    assert_eq!(engine.event_count().unwrap(), 1);

    let cleared = handle_ipc_value(&mut engine, json!({"type": "clear_events"}));
    assert_eq!(cleared["type"], "cleared");
    assert_eq!(cleared["deleted"], 1);
    assert_eq!(engine.event_count().unwrap(), 0);

    let stats = handle_ipc_value(&mut engine, json!({"type": "get_store_stats"}));
    assert_eq!(stats["type"], "store_stats");
    assert_eq!(stats["event_count"], 0);
    assert_eq!(stats["retention_days"], 30);
}

#[test]
fn set_retention_days_persists_and_reports_via_ipc() {
    let dir = tempdir().unwrap();
    let key = load_or_create_key(&dir.path().join("store.key")).unwrap();
    let store = EventStore::open(&dir.path().join("events.db"), &key).unwrap();
    let policy_path = dir.path().join("policy.json");
    let mut engine = Engine::from_policy_state(store, PolicyState::default(), policy_path.clone());

    let stats = handle_ipc_value(
        &mut engine,
        json!({"type": "set_retention_days", "days": 14}),
    );
    assert_eq!(stats["type"], "store_stats");
    assert_eq!(stats["retention_days"], 14);
    assert_eq!(engine.retention_days(), 14);

    let reloaded = cookie_monster_core::policy_state::load_policy_state(&policy_path).unwrap();
    assert_eq!(reloaded.retention_days, 14);
}
