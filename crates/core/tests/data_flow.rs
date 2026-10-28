
use cookie_monster_core::allowlist::{Allowlist, AllowlistEntry};
use cookie_monster_core::engine::Engine;
use cookie_monster_core::ipc::handle_ipc_value;
use cookie_monster_core::overrides::OverrideStore;
use cookie_monster_core::policy_state::{self, PolicyState};
use cookie_monster_core::store::EventStore;
use cookie_monster_core::types::*;
use serde_json::json;
use tempfile::tempdir;

fn make_event(id: &str, host: &str, name: &str, action: CookieAction, party: CookieParty) -> CookieEvent {
    CookieEvent {
        id: id.into(),
        ts_ms: 1_700_000_000_000,
        action,
        party,
        scheme: "https".into(),
        host: host.into(),
        name: name.into(),
        value_redacted: true,
        attributes: CookieAttributes {
            secure: true,
            http_only: false,
            same_site: Some("Lax".into()),
            max_age: Some(3600),
            domain: Some(host.into()),
            path: Some("/".into()),
        },
    }
}

#[test]
fn full_flow_observe_classifies_persists_and_lists_via_ipc() {
    let dir = tempdir().unwrap();
    let store = EventStore::open(&dir.path().join("events.db"), &[9u8; 32]).unwrap();
    let mut engine = Engine::new(Mode::Observe, store);

    let event = make_event("flow-1", "tracker.example", "_ga", CookieAction::Set, CookieParty::Third);
    let decision = engine.handle_event(event.clone()).unwrap();
    assert_eq!(decision.kind, DecisionKind::Allow);
    assert_eq!(decision.reason, "observe");

    let listed = handle_ipc_value(&mut engine, json!({"type": "list_events", "limit": 10}));
    assert_eq!(listed["type"], "events");
    assert_eq!(listed["items"][0]["event"]["id"], "flow-1");
    assert_eq!(listed["items"][0]["bucket"], "Analytics");
    assert_eq!(listed["items"][0]["decision"]["kind"], "Allow");
    let blob = serde_json::to_string(&listed).unwrap();
    assert!(!blob.contains("secret"));
}

#[test]
fn full_flow_override_changes_bucket_then_moderate_allowlists() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("events.db");
    let policy_path = dir.path().join("policy.json");
    let store = EventStore::open(&db, &[3u8; 32]).unwrap();

    let mut overrides = OverrideStore::default();
    overrides.set("shop.example", Some("_ga"), CookieBucket::Necessary);

    let state = PolicyState {
        mode: Mode::Moderate,
        allowlist: Allowlist::default(),
        overrides,
        system_shield: false,
        retention_days: cookie_monster_core::store::DEFAULT_RETENTION_DAYS,
    };
    policy_state::save_policy_state(&policy_path, &state).unwrap();
    let loaded = policy_state::load_policy_state(&policy_path).unwrap();
    let mut engine = Engine::from_policy_state(store, loaded, policy_path.clone());

    let decision = engine
        .handle_event(make_event(
            "flow-2",
            "shop.example",
            "_ga",
            CookieAction::Set,
            CookieParty::Third,
        ))
        .unwrap();
    assert_eq!(decision.kind, DecisionKind::Allow);
    assert_eq!(decision.reason, "moderate_necessary");

    let listed = engine.list_events(5).unwrap();
    assert_eq!(listed[0].bucket, CookieBucket::Necessary);
}

#[test]
fn full_flow_cookie_monster_denies_set_and_strips_send_through_ipc() {
    let dir = tempdir().unwrap();
    let store = EventStore::open(&dir.path().join("events.db"), &[1u8; 32]).unwrap();
    let mut engine = Engine::new(Mode::Observe, store);

    let status = handle_ipc_value(
        &mut engine,
        json!({"type": "set_mode", "mode": "cookie_monster"}),
    );
    assert_eq!(status["mode"], "cookie_monster");

    let deny = handle_ipc_value(
        &mut engine,
        json!({
            "type": "cookie_event",
            "event": make_event("cm-set", "a.com", "session", CookieAction::Set, CookieParty::First)
        }),
    );
    assert_eq!(deny["decision"]["kind"], "Deny");

    let strip = handle_ipc_value(
        &mut engine,
        json!({
            "type": "cookie_event",
            "event": make_event("cm-send", "a.com", "session", CookieAction::Send, CookieParty::First)
        }),
    );
    assert_eq!(strip["decision"]["kind"], "Strip");
}

#[test]
fn full_flow_allowlist_ipc_roundtrip_affects_moderate_decision() {
    let dir = tempdir().unwrap();
    let policy_path = dir.path().join("policy.json");
    let store = EventStore::open(&dir.path().join("events.db"), &[7u8; 32]).unwrap();
    let mut engine = Engine::from_policy_state(store, PolicyState::default(), policy_path);

    handle_ipc_value(&mut engine, json!({"type": "set_mode", "mode": "moderate"}));
    let set_list = handle_ipc_value(
        &mut engine,
        json!({
            "type": "set_allowlist",
            "entries": [{"host": "github.com", "allowed_buckets": null}]
        }),
    );
    assert_eq!(set_list["type"], "allowlist");
    assert_eq!(set_list["entries"][0]["host"], "github.com");

    let get_list = handle_ipc_value(&mut engine, json!({"type": "get_allowlist"}));
    assert_eq!(get_list["entries"].as_array().unwrap().len(), 1);

    let allowed = handle_ipc_value(
        &mut engine,
        json!({
            "type": "cookie_event",
            "event": make_event("al-1", "api.github.com", "_fbp", CookieAction::Set, CookieParty::Third)
        }),
    );
    assert_eq!(allowed["decision"]["kind"], "Allow");
    assert_eq!(allowed["decision"]["reason"], "allowlist");

    let blocked = handle_ipc_value(
        &mut engine,
        json!({
            "type": "cookie_event",
            "event": make_event("al-2", "ads.evil", "_fbp", CookieAction::Set, CookieParty::Third)
        }),
    );
    assert_eq!(blocked["decision"]["kind"], "Deny");
}

#[test]
fn full_flow_override_ipc_set_and_clear() {
    let dir = tempdir().unwrap();
    let policy_path = dir.path().join("policy.json");
    let store = EventStore::open(&dir.path().join("events.db"), &[5u8; 32]).unwrap();
    let mut engine = Engine::from_policy_state(store, PolicyState::default(), policy_path.clone());

    handle_ipc_value(&mut engine, json!({"type": "set_mode", "mode": "moderate"}));
    handle_ipc_value(
        &mut engine,
        json!({
            "type": "set_override",
            "host": "news.example",
            "name": "weird",
            "bucket": "Necessary"
        }),
    );

    let allowed = engine
        .handle_event(make_event(
            "ov-1",
            "news.example",
            "weird",
            CookieAction::Set,
            CookieParty::Third,
        ))
        .unwrap();
    assert_eq!(allowed.kind, DecisionKind::Allow);

    handle_ipc_value(
        &mut engine,
        json!({
            "type": "clear_override",
            "host": "news.example",
            "name": "weird"
        }),
    );

    let denied = engine
        .handle_event(make_event(
            "ov-2",
            "news.example",
            "weird",
            CookieAction::Set,
            CookieParty::Third,
        ))
        .unwrap();
    assert_eq!(denied.kind, DecisionKind::Deny);

    let reloaded = policy_state::load_policy_state(&policy_path).unwrap();
    assert_eq!(reloaded.mode, Mode::Moderate);
    assert!(reloaded.overrides.get("news.example", "weird").is_none());
}

#[test]
fn full_flow_bucket_filtered_allowlist_denies_wrong_type() {
    let dir = tempdir().unwrap();
    let store = EventStore::open(&dir.path().join("events.db"), &[2u8; 32]).unwrap();
    let state = PolicyState {
        mode: Mode::Moderate,
        allowlist: Allowlist::new(vec![AllowlistEntry {
            host: "news.example".into(),
            allowed_buckets: Some(vec![CookieBucket::Necessary]),
        }]),
        overrides: OverrideStore::default(),
        system_shield: false,
        retention_days: cookie_monster_core::store::DEFAULT_RETENTION_DAYS,
    };
    let mut engine = Engine::from_policy_state(store, state, dir.path().join("policy.json"));

    let denied = engine
        .handle_event(make_event(
            "bf-1",
            "news.example",
            "_ga",
            CookieAction::Set,
            CookieParty::First,
        ))
        .unwrap();
    assert_eq!(denied.kind, DecisionKind::Deny);
    assert_eq!(denied.reason, "allowlist_type_blocked");
}
