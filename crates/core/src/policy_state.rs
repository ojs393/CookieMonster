use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::allowlist::Allowlist;
use crate::overrides::OverrideStore;
use crate::store::DEFAULT_RETENTION_DAYS;
use crate::types::Mode;

pub const POLICY_FILE_NAME: &str = "policy.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyState {
    pub mode: Mode,
    pub allowlist: Allowlist,
    pub overrides: OverrideStore,
    #[serde(default)]
    pub system_shield: bool,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_retention_days() -> u32 {
    DEFAULT_RETENTION_DAYS
}

impl Default for PolicyState {
    fn default() -> Self {
        Self {
            mode: Mode::Observe,
            allowlist: Allowlist::default(),
            overrides: OverrideStore::default(),
            system_shield: false,
            retention_days: DEFAULT_RETENTION_DAYS,
        }
    }
}

#[derive(Debug, Error)]
pub enum PolicyStateError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn load_policy_state(path: &Path) -> Result<PolicyState, PolicyStateError> {
    match fs::read_to_string(path) {
        Ok(raw) => Ok(serde_json::from_str(&raw)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(PolicyState::default()),
        Err(error) => Err(error.into()),
    }
}

pub fn save_policy_state(path: &Path, state: &PolicyState) -> Result<(), PolicyStateError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_vec_pretty(state)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &raw)?;
    fs::rename(&tmp, path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
