use cookie_monster_core::allowlist::{host_matches, Allowlist, AllowlistEntry};
use cookie_monster_core::overrides::OverrideStore;
use cookie_monster_core::types::CookieBucket;

#[test]
fn host_matching_covers_apex_subdomain_and_negatives() {
    assert!(host_matches("Example.COM", "example.com"));
    assert!(host_matches("example.com", ".www.example.com"));
    assert!(host_matches(".example.com", "a.b.example.com"));
    assert!(!host_matches("example.com", "example.com.evil"));
    assert!(!host_matches("ample.com", "example.com"));
    assert!(!host_matches("", "example.com"));
    assert!(!host_matches("example.com", ""));
}

#[test]
fn allowlist_decision_for_all_buckets_and_filtered() {
    let all = Allowlist::new(vec![AllowlistEntry {
        host: "ok.com".into(),
        allowed_buckets: None,
    }]);
    assert_eq!(all.decision_for("www.ok.com", CookieBucket::Advertising), Some(true));

    let filtered = Allowlist::new(vec![AllowlistEntry {
        host: "ok.com".into(),
        allowed_buckets: Some(vec![CookieBucket::Necessary]),
    }]);
    assert_eq!(
        filtered.decision_for("ok.com", CookieBucket::Necessary),
        Some(true)
    );
    assert_eq!(
        filtered.decision_for("ok.com", CookieBucket::Analytics),
        Some(false)
    );
    assert_eq!(
        filtered.decision_for("other.com", CookieBucket::Necessary),
        None
    );
}

#[test]
fn override_name_beats_host_level_and_clear_works() {
    let mut store = OverrideStore::default();
    store.set("a.com", None, CookieBucket::Advertising);
    store.set("a.com", Some("session"), CookieBucket::Necessary);

    assert_eq!(store.get("a.com", "session"), Some(CookieBucket::Necessary));
    assert_eq!(store.get("a.com", "other"), Some(CookieBucket::Advertising));

    store.clear("a.com", Some("session"));
    assert_eq!(store.get("a.com", "session"), Some(CookieBucket::Advertising));

    store.clear("a.com", None);
    assert_eq!(store.get("a.com", "other"), None);
}
