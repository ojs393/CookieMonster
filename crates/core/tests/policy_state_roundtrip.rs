use cookie_monster_core::allowlist::{Allowlist, AllowlistEntry};
use cookie_monster_core::overrides::OverrideStore;
use cookie_monster_core::policy_state::{self, PolicyState};
use cookie_monster_core::types::{CookieBucket, Mode};
use tempfile::tempdir;

#[test]
fn missing_policy_file_loads_defaults() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("missing-policy.json");
    let state = policy_state::load_policy_state(&path).unwrap();
    assert_eq!(state.mode, Mode::Observe);
    assert!(state.allowlist.entries.is_empty());
    assert!(state.overrides.map.is_empty());
    assert_eq!(
        state.retention_days,
        cookie_monster_core::store::DEFAULT_RETENTION_DAYS
    );
}

#[test]
fn save_and_load_preserves_mode_allowlist_and_overrides() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("policy.json");

    let mut overrides = OverrideStore::default();
    overrides.set("x.com", Some("sid"), CookieBucket::Necessary);

    let original = PolicyState {
        mode: Mode::CookieMonster,
        allowlist: Allowlist::new(vec![AllowlistEntry {
            host: "github.com".into(),
            allowed_buckets: Some(vec![CookieBucket::Necessary, CookieBucket::Preferences]),
        }]),
        overrides,
        system_shield: true,
        retention_days: 14,
    };
    policy_state::save_policy_state(&path, &original).unwrap();
    let loaded = policy_state::load_policy_state(&path).unwrap();
    assert_eq!(loaded, original);
}

#[test]
fn save_creates_parent_directories() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nested").join("dir").join("policy.json");
    policy_state::save_policy_state(&path, &PolicyState::default()).unwrap();
    assert!(path.is_file());
}

#[test]
fn legacy_policy_without_retention_defaults() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("policy.json");
    std::fs::write(
        &path,
        r#"{"mode":"observe","allowlist":{"entries":[]},"overrides":{"map":{}},"system_shield":false}"#,
    )
    .unwrap();
    let state = policy_state::load_policy_state(&path).unwrap();
    assert_eq!(
        state.retention_days,
        cookie_monster_core::store::DEFAULT_RETENTION_DAYS
    );
}

#[cfg(unix)]
#[test]
fn saved_policy_is_owner_readable_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let path = dir.path().join("policy.json");
    policy_state::save_policy_state(&path, &PolicyState::default()).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
