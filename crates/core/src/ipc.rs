use std::io::{Read, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::allowlist::{Allowlist, AllowlistEntry};
use crate::engine::Engine;
use crate::store::StoredEvent;
use crate::types::{CookieBucket, CookieEvent, Decision, Mode};

pub const MAX_INBOUND_MESSAGE_SIZE: usize = 64 * 1024 * 1024;
pub const MAX_OUTBOUND_MESSAGE_SIZE: usize = 1024 * 1024;
pub const MAX_LIST_EVENTS: usize = 1000;

pub type Result<T> = std::result::Result<T, IpcError>;

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("native message is too large")]
    MessageTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    CookieEvent { event: CookieEvent },
    GetStatus,
    SetMode { mode: Mode },
    ListEvents { limit: usize },
    GetAllowlist,
    SetAllowlist { entries: Vec<AllowlistEntry> },
    SetOverride {
        host: String,
        name: Option<String>,
        bucket: CookieBucket,
    },
    ClearOverride {
        host: String,
        name: Option<String>,
    },
    SetSystemShield { enabled: bool },
    SetRetentionDays { days: u32 },
    ClearEvents,
    GetStoreStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Decision {
        decision: Decision,
    },
    Status {
        mode: Mode,
        browser_shield: bool,
        system_shield: bool,
    },
    Events {
        items: Vec<StoredEvent>,
    },
    Allowlist {
        entries: Vec<AllowlistEntry>,
    },
    Cleared {
        deleted: u64,
    },
    StoreStats {
        event_count: u64,
        retention_days: u32,
    },
    Error {
        message: String,
    },
}

pub type IpcRequest = Request;
pub type IpcResponse = Response;

pub fn handle_ipc_value(engine: &mut Engine, value: Value) -> Value {
    let response = match serde_json::from_value::<Request>(value) {
        Ok(Request::CookieEvent { event }) => match engine.handle_event(event) {
            Ok(decision) => Response::Decision { decision },
            Err(error) => Response::Error {
                message: error.to_string(),
            },
        },
        Ok(Request::GetStatus) => status_response(engine),
        Ok(Request::SetMode { mode }) => match engine.set_mode(mode) {
            Ok(()) => status_response(engine),
            Err(error) => Response::Error {
                message: error.to_string(),
            },
        },
        Ok(Request::ListEvents { limit }) => match engine.list_events(limit.min(MAX_LIST_EVENTS)) {
            Ok(items) => Response::Events { items },
            Err(error) => Response::Error {
                message: error.to_string(),
            },
        },
        Ok(Request::GetAllowlist) => Response::Allowlist {
            entries: engine.allowlist().entries.clone(),
        },
        Ok(Request::SetAllowlist { entries }) => match engine.set_allowlist(Allowlist::new(entries))
        {
            Ok(()) => Response::Allowlist {
                entries: engine.allowlist().entries.clone(),
            },
            Err(error) => Response::Error {
                message: error.to_string(),
            },
        },
        Ok(Request::SetOverride { host, name, bucket }) => {
            match engine.set_override(host, name, bucket) {
                Ok(()) => status_response(engine),
                Err(error) => Response::Error {
                    message: error.to_string(),
                },
            }
        }
        Ok(Request::ClearOverride { host, name }) => match engine.clear_override(host, name) {
            Ok(()) => status_response(engine),
            Err(error) => Response::Error {
                message: error.to_string(),
            },
        },
        Ok(Request::SetSystemShield { enabled }) => match engine.set_system_shield(enabled) {
            Ok(()) => status_response(engine),
            Err(error) => Response::Error {
                message: error.to_string(),
            },
        },
        Ok(Request::SetRetentionDays { days }) => match engine.set_retention_days(days) {
            Ok(()) => match engine.event_count() {
                Ok(event_count) => Response::StoreStats {
                    event_count,
                    retention_days: engine.retention_days(),
                },
                Err(error) => Response::Error {
                    message: error.to_string(),
                },
            },
            Err(error) => Response::Error {
                message: error.to_string(),
            },
        },
        Ok(Request::ClearEvents) => match engine.clear_events() {
            Ok(deleted) => Response::Cleared { deleted },
            Err(error) => Response::Error {
                message: error.to_string(),
            },
        },
        Ok(Request::GetStoreStats) => match engine.event_count() {
            Ok(event_count) => Response::StoreStats {
                event_count,
                retention_days: engine.retention_days(),
            },
            Err(error) => Response::Error {
                message: error.to_string(),
            },
        },
        Err(error) => Response::Error {
            message: error.to_string(),
        },
    };

    serde_json::to_value(response).expect("IPC responses must serialize")
}

fn status_response(engine: &Engine) -> Response {
    Response::Status {
        mode: engine.mode(),
        browser_shield: true,
        system_shield: engine.system_shield(),
    }
}

pub fn encode_native_message(value: &impl Serialize) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(value)?;
    if payload.len() > MAX_OUTBOUND_MESSAGE_SIZE {
        return Err(IpcError::MessageTooLarge);
    }
    let length = u32::try_from(payload.len()).map_err(|_| IpcError::MessageTooLarge)?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.write_all(&length.to_le_bytes())?;
    frame.write_all(&payload)?;
    Ok(frame)
}

pub fn decode_native_message(reader: &mut impl Read) -> Result<Value> {
    let mut length = [0u8; 4];
    reader.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_INBOUND_MESSAGE_SIZE {
        return Err(IpcError::MessageTooLarge);
    }
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload)?;
    Ok(serde_json::from_slice(&payload)?)
}
