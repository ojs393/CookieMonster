use std::io::Cursor;

use cookie_monster_core::ipc::{
    decode_native_message, encode_native_message, IpcError, Request, Response,
};
use cookie_monster_core::types::Mode;
use serde_json::json;

#[test]
fn native_message_encode_decode_roundtrips_json() {
    let value = json!({"type": "get_status"});

    let encoded = encode_native_message(&value).unwrap();

    assert_eq!(
        u32::from_le_bytes(encoded[..4].try_into().unwrap()) as usize,
        encoded.len() - 4
    );
    let decoded = decode_native_message(&mut Cursor::new(encoded)).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn decode_rejects_oversized_message_before_reading_payload() {
    let oversized_length = (64 * 1024 * 1024_u32) + 1;
    let error =
        decode_native_message(&mut Cursor::new(oversized_length.to_le_bytes())).unwrap_err();

    assert!(matches!(error, IpcError::MessageTooLarge));
}

#[test]
fn encode_rejects_message_larger_than_native_host_outbound_limit() {
    let oversized_value = "x".repeat(1024 * 1024);

    let error = encode_native_message(&oversized_value).unwrap_err();

    assert!(matches!(error, IpcError::MessageTooLarge));
}

#[test]
fn request_types_and_modes_use_protocol_wire_names() {
    let cases = [
        (Request::GetStatus, json!({"type": "get_status"})),
        (
            Request::SetMode {
                mode: Mode::Observe,
            },
            json!({"type": "set_mode", "mode": "observe"}),
        ),
        (
            Request::SetMode {
                mode: Mode::Moderate,
            },
            json!({"type": "set_mode", "mode": "moderate"}),
        ),
        (
            Request::SetMode {
                mode: Mode::CookieMonster,
            },
            json!({"type": "set_mode", "mode": "cookie_monster"}),
        ),
        (
            Request::ListEvents { limit: 50 },
            json!({"type": "list_events", "limit": 50}),
        ),
        (
            Request::GetAllowlist,
            json!({"type": "get_allowlist"}),
        ),
        (
            Request::SetAllowlist {
                entries: vec![],
            },
            json!({"type": "set_allowlist", "entries": []}),
        ),
        (
            Request::SetOverride {
                host: "a.com".into(),
                name: Some("sid".into()),
                bucket: cookie_monster_core::types::CookieBucket::Necessary,
            },
            json!({
                "type": "set_override",
                "host": "a.com",
                "name": "sid",
                "bucket": "Necessary"
            }),
        ),
        (
            Request::ClearOverride {
                host: "a.com".into(),
                name: None,
            },
            json!({"type": "clear_override", "host": "a.com", "name": null}),
        ),
    ];

    for (request, expected) in cases {
        assert_eq!(serde_json::to_value(request).unwrap(), expected);
    }
}

#[test]
fn status_response_matches_protocol_shape() {
    let response = Response::Status {
        mode: Mode::Observe,
        browser_shield: true,
        system_shield: false,
    };

    assert_eq!(
        serde_json::to_value(response).unwrap(),
        json!({
            "type": "status",
            "mode": "observe",
            "browser_shield": true,
            "system_shield": false
        })
    );
}
