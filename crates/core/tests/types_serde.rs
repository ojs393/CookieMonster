use cookie_monster_core::types::*;
use serde_json::json;

#[test]
fn default_mode_is_observe() {
    assert_eq!(DEFAULT_MODE, Mode::Observe);
}

#[test]
fn cookie_event_roundtrips_json() {
    let ev = CookieEvent {
        id: "evt_1".into(),
        ts_ms: 1_700_000_000_000,
        action: CookieAction::Set,
        party: CookieParty::First,
        scheme: "https".into(),
        host: "example.com".into(),
        name: "session".into(),
        value_redacted: true,
        attributes: Default::default(),
    };
    let json = serde_json::to_string(&ev).unwrap();
    let back: CookieEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(back.host, "example.com");
    assert_eq!(back.action, CookieAction::Set);
}

#[test]
fn mode_wire_names_are_snake_case() {
    assert_eq!(serde_json::to_value(Mode::Observe).unwrap(), json!("observe"));
    assert_eq!(serde_json::to_value(Mode::Moderate).unwrap(), json!("moderate"));
    assert_eq!(
        serde_json::to_value(Mode::CookieMonster).unwrap(),
        json!("cookie_monster")
    );
}

#[test]
fn action_party_bucket_decision_use_pascal_case() {
    assert_eq!(serde_json::to_value(CookieAction::Send).unwrap(), json!("Send"));
    assert_eq!(serde_json::to_value(CookieAction::Js).unwrap(), json!("Js"));
    assert_eq!(serde_json::to_value(CookieParty::Third).unwrap(), json!("Third"));
    assert_eq!(
        serde_json::to_value(CookieBucket::Advertising).unwrap(),
        json!("Advertising")
    );
    assert_eq!(serde_json::to_value(DecisionKind::Strip).unwrap(), json!("Strip"));
}

#[test]
fn empty_attributes_object_deserializes_with_defaults() {
    let ev: CookieEvent = serde_json::from_value(json!({
        "id": "x",
        "ts_ms": 1,
        "action": "Set",
        "party": "First",
        "scheme": "https",
        "host": "a.com",
        "name": "n",
        "value_redacted": true,
        "attributes": {}
    }))
    .unwrap();
    assert!(!ev.attributes.secure);
    assert!(ev.attributes.same_site.is_none());
}
