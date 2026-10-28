use cookie_monster_core::allowlist::{Allowlist, AllowlistEntry};
use cookie_monster_core::policy::decide;
use cookie_monster_core::types::*;

fn event(action: CookieAction, party: CookieParty, host: &str, name: &str) -> CookieEvent {
    CookieEvent {
        id: "e1".into(),
        ts_ms: 0,
        action,
        party,
        scheme: "https".into(),
        host: host.into(),
        name: name.into(),
        value_redacted: true,
        attributes: Default::default(),
    }
}

#[test]
fn observe_always_allows() {
    let empty = Allowlist::default();
    let d = decide(
        Mode::Observe,
        &event(CookieAction::Set, CookieParty::Third, "tracker.test", "_ga"),
        CookieBucket::Advertising,
        &empty,
    );
    assert_eq!(d.kind, DecisionKind::Allow);
    assert_eq!(d.reason, "observe");
}

#[test]
fn cookie_monster_denies_set_and_strips_send_and_js() {
    let empty = Allowlist::default();
    let deny = decide(
        Mode::CookieMonster,
        &event(CookieAction::Set, CookieParty::First, "a.com", "x"),
        CookieBucket::Necessary,
        &empty,
    );
    assert_eq!(deny.kind, DecisionKind::Deny);
    let strip = decide(
        Mode::CookieMonster,
        &event(CookieAction::Send, CookieParty::First, "a.com", "x"),
        CookieBucket::Necessary,
        &empty,
    );
    assert_eq!(strip.kind, DecisionKind::Strip);
    let strip_js = decide(
        Mode::CookieMonster,
        &event(CookieAction::Js, CookieParty::First, "a.com", "x"),
        CookieBucket::Necessary,
        &empty,
    );
    assert_eq!(strip_js.kind, DecisionKind::Strip);
}

#[test]
fn moderate_allows_necessary_denies_unknown_and_ads() {
    let empty = Allowlist::default();
    let necessary = decide(
        Mode::Moderate,
        &event(CookieAction::Set, CookieParty::First, "a.com", "session"),
        CookieBucket::Necessary,
        &empty,
    );
    assert_eq!(necessary.kind, DecisionKind::Allow);

    let prefs = decide(
        Mode::Moderate,
        &event(CookieAction::Set, CookieParty::First, "a.com", "lang"),
        CookieBucket::Preferences,
        &empty,
    );
    assert_eq!(prefs.kind, DecisionKind::Allow);

    let third_prefs = decide(
        Mode::Moderate,
        &event(CookieAction::Set, CookieParty::Third, "ads.com", "lang"),
        CookieBucket::Preferences,
        &empty,
    );
    assert_eq!(third_prefs.kind, DecisionKind::Deny);

    let ads = decide(
        Mode::Moderate,
        &event(CookieAction::Set, CookieParty::Third, "ads.com", "_fbp"),
        CookieBucket::Advertising,
        &empty,
    );
    assert_eq!(ads.kind, DecisionKind::Deny);
    assert_eq!(ads.reason, "moderate_blocked");
}

#[test]
fn moderate_allowlist_permits_site_and_respects_bucket_filter() {
    let list = Allowlist::new(vec![
        AllowlistEntry {
            host: "github.com".into(),
            allowed_buckets: None,
        },
        AllowlistEntry {
            host: "news.example".into(),
            allowed_buckets: Some(vec![CookieBucket::Necessary, CookieBucket::Preferences]),
        },
    ]);

    let allowed = decide(
        Mode::Moderate,
        &event(CookieAction::Set, CookieParty::Third, "api.github.com", "_ga"),
        CookieBucket::Analytics,
        &list,
    );
    assert_eq!(allowed.kind, DecisionKind::Allow);
    assert_eq!(allowed.reason, "allowlist");

    let filtered_out = decide(
        Mode::Moderate,
        &event(CookieAction::Set, CookieParty::First, "news.example", "_ga"),
        CookieBucket::Analytics,
        &list,
    );
    assert_eq!(filtered_out.kind, DecisionKind::Deny);
    assert_eq!(filtered_out.reason, "allowlist_type_blocked");
}
