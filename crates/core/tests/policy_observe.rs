use cookie_monster_core::allowlist::Allowlist;
use cookie_monster_core::policy::decide;
use cookie_monster_core::types::*;

fn sample_event() -> CookieEvent {
    CookieEvent {
        id: "e1".into(),
        ts_ms: 0,
        action: CookieAction::Set,
        party: CookieParty::Third,
        scheme: "https".into(),
        host: "tracker.example".into(),
        name: "_ga".into(),
        value_redacted: true,
        attributes: Default::default(),
    }
}

#[test]
fn observe_always_allows_even_third_party_tracker() {
    let d = decide(
        Mode::Observe,
        &sample_event(),
        CookieBucket::Advertising,
        &Allowlist::default(),
    );
    assert_eq!(d.kind, DecisionKind::Allow);
    assert!(d.reason.contains("observe") || d.reason.contains("Observe"));
}
