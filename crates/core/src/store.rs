use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use chrono::Utc;
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::types::{CookieBucket, CookieEvent, Decision};

const NONCE_LEN: usize = 12;
pub const DEFAULT_RETENTION_DAYS: u32 = 30;
pub const MAX_STORED_EVENTS: u64 = 50_000;

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("store file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("event serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("event encryption failed")]
    Encryption,
    #[error("event decryption failed")]
    Decryption,
    #[error("invalid nonce length: {0}")]
    InvalidNonceLength(usize),
    #[error("invalid store key length: {0}")]
    InvalidKeyLength(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEvent {
    pub event: CookieEvent,
    pub decision: Decision,
    pub bucket: CookieBucket,
    pub stored_at_ms: i64,
}

#[derive(Serialize, Deserialize)]
struct StoredPayload {
    event: CookieEvent,
    decision: Decision,
    bucket: CookieBucket,
}

pub struct EventStore {
    connection: Connection,
    key: Zeroizing<[u8; 32]>,
}

pub fn load_or_create_key(path: &Path) -> Result<[u8; 32]> {
    let mut generated = [0u8; 32];
    OsRng.fill_bytes(&mut generated);

    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(ErrorKind::InvalidInput, "store key path has no parent")
    })?;
    let key_name = path
        .file_name()
        .ok_or_else(|| {
            std::io::Error::new(ErrorKind::InvalidInput, "store key path has no file name")
        })?
        .to_string_lossy();

    let (temp_path, mut temp_file) = loop {
        let temp_path = parent.join(format!(
            ".{key_name}.tmp-{}-{:016x}",
            std::process::id(),
            OsRng.next_u64()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);

        match options.open(&temp_path) {
            Ok(file) => break (temp_path, file),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    };

    temp_file.write_all(&generated)?;
    temp_file.sync_all()?;
    drop(temp_file);

    let publication = publish_key(&temp_path, path);
    if let Err(error) = fs::remove_file(&temp_path) {
        if publication.is_ok() {
            return Err(error.into());
        }
    }
    publication?;

    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;

    let bytes = fs::read(path)?;
    bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| StoreError::InvalidKeyLength(bytes.len()))
}

fn publish_key(temp_path: &Path, final_path: &Path) -> Result<bool> {
    match fs::hard_link(temp_path, final_path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.into()),
    }
}

impl EventStore {
    pub fn open(path: &Path, key: &[u8; 32]) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA secure_delete=ON;
             CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                ts_ms INTEGER NOT NULL,
                nonce BLOB NOT NULL,
                ciphertext BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS events_ts_ms_idx ON events(ts_ms);",
        )?;
        #[cfg(unix)]
        {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }

        Ok(Self {
            connection,
            key: Zeroizing::new(*key),
        })
    }

    pub fn append(
        &self,
        event: &CookieEvent,
        decision: &Decision,
        bucket: CookieBucket,
    ) -> Result<()> {
        let mut redacted_event = event.clone();
        redacted_event.value_redacted = true;
        let payload = StoredPayload {
            event: redacted_event,
            decision: decision.clone(),
            bucket,
        };
        let plaintext = serde_json::to_vec(&payload)?;
        let cipher = Aes256Gcm::new_from_slice(self.key.as_ref())
            .expect("a 32-byte key is always valid for AES-256-GCM");
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_ref())
            .map_err(|_| StoreError::Encryption)?;
        let stored_at_ms = Utc::now().timestamp_millis();

        self.connection.execute(
            "INSERT INTO events (id, ts_ms, nonce, ciphertext)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO NOTHING",
            params![event.id, stored_at_ms, nonce_bytes, ciphertext],
        )?;
        Ok(())
    }

    pub fn list_recent(&self, limit: usize) -> Result<Vec<StoredEvent>> {
        let mut statement = self.connection.prepare(
            "SELECT ts_ms, nonce, ciphertext
             FROM events
             ORDER BY ts_ms DESC, rowid DESC
             LIMIT ?1",
        )?;
        let encrypted_rows = statement
            .query_map(params![limit as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let cipher = Aes256Gcm::new_from_slice(self.key.as_ref())
            .expect("a 32-byte key is always valid for AES-256-GCM");
        encrypted_rows
            .into_iter()
            .map(|(stored_at_ms, nonce, ciphertext)| {
                if nonce.len() != NONCE_LEN {
                    return Err(StoreError::InvalidNonceLength(nonce.len()));
                }
                let plaintext = cipher
                    .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
                    .map_err(|_| StoreError::Decryption)?;
                let payload: StoredPayload = serde_json::from_slice(&plaintext)?;
                Ok(StoredEvent {
                    event: payload.event,
                    decision: payload.decision,
                    bucket: payload.bucket,
                    stored_at_ms,
                })
            })
            .collect()
    }

    pub fn prune_older_than_days(&self, days: u32) -> Result<u64> {
        let cutoff_ms = Utc::now().timestamp_millis() - i64::from(days) * 86_400_000;
        let deleted = self
            .connection
            .execute("DELETE FROM events WHERE ts_ms < ?1", [cutoff_ms])?;
        if deleted > 0 {
            let _ = self.connection.execute_batch("PRAGMA incremental_vacuum(64);");
        }
        Ok(deleted as u64)
    }

    pub fn clear_all(&self) -> Result<u64> {
        let deleted = self.connection.execute("DELETE FROM events", [])?;
        let _ = self.connection.execute_batch("VACUUM;");
        Ok(deleted as u64)
    }

    pub fn event_count(&self) -> Result<u64> {
        let count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    /// Age prune + hard cap + light vacuum. Safe to call periodically.
    pub fn enforce_limits(&self) -> Result<u64> {
        self.enforce_limits_configured(DEFAULT_RETENTION_DAYS, MAX_STORED_EVENTS)
    }

    pub fn enforce_limits_with_retention(&self, retention_days: u32) -> Result<u64> {
        self.enforce_limits_configured(retention_days, MAX_STORED_EVENTS)
    }

    pub fn enforce_limits_configured(
        &self,
        retention_days: u32,
        max_events: u64,
    ) -> Result<u64> {
        let days = if retention_days == 0 {
            DEFAULT_RETENTION_DAYS
        } else {
            retention_days
        };
        let cap = if max_events == 0 {
            MAX_STORED_EVENTS
        } else {
            max_events
        };
        let mut deleted = self.prune_older_than_days(days)?;
        let count = self.event_count()?;
        if count > cap {
            let excess = (count - cap) as i64;
            let n = self.connection.execute(
                "DELETE FROM events WHERE rowid IN (
                    SELECT rowid FROM events ORDER BY ts_ms ASC, rowid ASC LIMIT ?1
                 )",
                [excess],
            )?;
            deleted += n as u64;
            let _ = self.connection.execute_batch("PRAGMA incremental_vacuum(128);");
        }
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::publish_key;
    use tempfile::tempdir;

    #[test]
    fn key_publication_is_atomic_and_does_not_replace_a_winner() {
        let dir = tempdir().unwrap();
        let final_path = dir.path().join("store.key");
        let first_temp = dir.path().join("first.tmp");
        let second_temp = dir.path().join("second.tmp");
        std::fs::write(&first_temp, [1u8; 32]).unwrap();
        std::fs::write(&second_temp, [2u8; 32]).unwrap();

        assert!(publish_key(&first_temp, &final_path).unwrap());
        assert!(!publish_key(&second_temp, &final_path).unwrap());
        assert_eq!(std::fs::read(final_path).unwrap(), [1u8; 32]);
    }
}
