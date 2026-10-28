use cookie_monster_core::classify::classify;
use cookie_monster_core::types::*;

fn ev(name: &str) -> CookieEvent {
    CookieEvent {
        id: "e".into(),
        ts_ms: 0,
        action: CookieAction::Set,
        party: CookieParty::First,
        scheme: "https".into(),
        host: "example.com".into(),
        name: name.into(),
        value_redacted: true,
        attributes: Default::default(),
    }
}

#[test]
fn analytics_names() {
    for name in ["_ga", "_gid", "_gat", "_ga_ABC"] {
        assert_eq!(classify(&ev(name)), CookieBucket::Analytics, "{name}");
    }
}

#[test]
fn advertising_names() {
    for name in ["_fbp", "IDE", "fr", "_gcl_au", "_fbp_extra"] {
        assert_eq!(classify(&ev(name)), CookieBucket::Advertising, "{name}");
    }
}

#[test]
fn necessary_names() {
    for name in ["session", "sid", "csrf", "csrftoken", "sid-extra"] {
        assert_eq!(classify(&ev(name)), CookieBucket::Necessary, "{name}");
    }
}

#[test]
fn preferences_names() {
    for name in ["lang", "locale", "theme", "tz"] {
        assert_eq!(classify(&ev(name)), CookieBucket::Preferences, "{name}");
    }
}

#[test]
fn unknown_name_is_unknown() {
    assert_eq!(classify(&ev("random_xyz_123")), CookieBucket::Unknown);
    assert_eq!(classify(&ev("from")), CookieBucket::Unknown);
}
