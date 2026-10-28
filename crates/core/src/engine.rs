use std::path::{Path, PathBuf};

use crate::allowlist::{Allowlist, AllowlistEntry};
use crate::classify::classify;
use crate::overrides::OverrideStore;
use crate::policy::decide;
use crate::policy_state::{self, PolicyState, PolicyStateError};
use crate::store::{EventStore, Result as StoreResult, StoredEvent};
use crate::types::{CookieBucket, CookieEvent, Decision, Mode};

pub struct Engine {
    mode: Mode,
    store: EventStore,
    allowlist: Allowlist,
    overrides: OverrideStore,
    system_shield: bool,
    retention_days: u32,
    policy_path: Option<PathBuf>,
}

impl Engine {
    pub fn new(mode: Mode, store: EventStore) -> Self {
        Self {
            mode,
            store,
            allowlist: Allowlist::default(),
            overrides: OverrideStore::default(),
            system_shield: false,
            retention_days: crate::store::DEFAULT_RETENTION_DAYS,
            policy_path: None,
        }
    }

    pub fn from_policy_state(store: EventStore, state: PolicyState, policy_path: PathBuf) -> Self {
        Self {
            mode: state.mode,
            store,
            allowlist: state.allowlist,
            overrides: state.overrides,
            system_shield: state.system_shield,
            retention_days: if state.retention_days == 0 {
                crate::store::DEFAULT_RETENTION_DAYS
            } else {
                state.retention_days
            },
            policy_path: Some(policy_path),
        }
    }

    pub fn handle_event(&mut self, event: CookieEvent) -> StoreResult<Decision> {
        let bucket = self.effective_bucket(&event);
        let decision = decide(self.mode, &event, bucket, &self.allowlist);
        self.store.append(&event, &decision, bucket)?;
        Ok(decision)
    }

    fn effective_bucket(&self, event: &CookieEvent) -> CookieBucket {
        self.overrides
            .get(&event.host, &event.name)
            .unwrap_or_else(|| classify(event))
    }

    pub fn set_mode(&mut self, mode: Mode) -> Result<(), PolicyStateError> {
        self.mode = mode;
        self.persist()
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn system_shield(&self) -> bool {
        self.system_shield
    }

    pub fn set_system_shield(&mut self, enabled: bool) -> Result<(), PolicyStateError> {
        self.system_shield = enabled;
        self.persist()
    }

    pub fn allowlist(&self) -> &Allowlist {
        &self.allowlist
    }

    pub fn set_allowlist(&mut self, allowlist: Allowlist) -> Result<(), PolicyStateError> {
        self.allowlist = allowlist;
        self.persist()
    }

    pub fn set_allowlist_entries(
        &mut self,
        entries: Vec<AllowlistEntry>,
    ) -> Result<(), PolicyStateError> {
        self.set_allowlist(Allowlist::new(entries))
    }

    pub fn set_override(
        &mut self,
        host: String,
        name: Option<String>,
        bucket: CookieBucket,
    ) -> Result<(), PolicyStateError> {
        self.overrides.set(&host, name.as_deref(), bucket);
        self.persist()
    }

    pub fn clear_override(
        &mut self,
        host: String,
        name: Option<String>,
    ) -> Result<(), PolicyStateError> {
        self.overrides.clear(&host, name.as_deref());
        self.persist()
    }

    pub fn list_events(&self, limit: usize) -> StoreResult<Vec<StoredEvent>> {
        self.store.list_recent(limit)
    }

    pub fn clear_events(&self) -> StoreResult<u64> {
        self.store.clear_all()
    }

    pub fn event_count(&self) -> StoreResult<u64> {
        self.store.event_count()
    }

    pub fn retention_days(&self) -> u32 {
        self.retention_days
    }

    pub fn set_retention_days(&mut self, days: u32) -> Result<(), PolicyStateError> {
        self.retention_days = if days == 0 {
            crate::store::DEFAULT_RETENTION_DAYS
        } else {
            days
        };
        self.persist()
    }

    pub fn prune_events(&self, days: u32) -> StoreResult<u64> {
        self.store.prune_older_than_days(days)
    }

    pub fn enforce_store_limits(&self) -> StoreResult<u64> {
        self.store
            .enforce_limits_with_retention(self.retention_days)
    }

    fn persist(&self) -> Result<(), PolicyStateError> {
        let Some(path) = self.policy_path.as_ref() else {
            return Ok(());
        };
        policy_state::save_policy_state(
            path,
            &PolicyState {
                mode: self.mode,
                allowlist: self.allowlist.clone(),
                overrides: self.overrides.clone(),
                system_shield: self.system_shield,
                retention_days: self.retention_days,
            },
        )
    }

    pub fn policy_path(&self) -> Option<&Path> {
        self.policy_path.as_deref()
    }
}
