//! The wire contract between the session commands and the React view.
//!
//! The payload types live in `orquestra-sessions`, but the frozen shape
//! fixture belongs to the app's IPC contract (`tests/fixtures/`), so the
//! guard stays here: a field added, renamed or dropped from a session payload
//! breaks this test before it reaches the UI.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

use super::types::{
    PinnedSession, PurgeReport, RetentionPolicy, SessionFilter, TranscriptTurn, UnifiedSession,
};

fn keys_of<T: Serialize>(value: &T) -> BTreeSet<String> {
    match serde_json::to_value(value).expect("value should serialize") {
        Value::Object(map) => map.keys().cloned().collect(),
        other => panic!("expected an object payload, got {other}"),
    }
}

fn frozen(shape: &str) -> BTreeSet<String> {
    let raw = include_str!("../../tests/fixtures/ipc-contract.json");
    let fixture: Value = serde_json::from_str(raw).expect("the IPC fixture should be valid JSON");

    fixture["payloadShapes"][shape]
        .as_array()
        .unwrap_or_else(|| panic!("fixture has no payloadShapes.{shape}"))
        .iter()
        .map(|key| key.as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn session_payloads_match_the_frozen_ipc_contract() {
    assert_eq!(
        keys_of(&UnifiedSession::new(
            "codex",
            "01f1",
            "Kernel audio".to_string()
        )),
        frozen("UnifiedSession")
    );
    assert_eq!(keys_of(&SessionFilter::default()), frozen("SessionFilter"));
    assert_eq!(
        keys_of(&RetentionPolicy::default()),
        frozen("RetentionPolicy")
    );
    assert_eq!(
        keys_of(&PinnedSession {
            harness: "codex".to_string(),
            session_id: "01f1".to_string(),
            pinned_at: "2026-10-18T06:00:00Z".to_string(),
            label: None,
        }),
        frozen("PinnedSession")
    );
    assert_eq!(keys_of(&PurgeReport::default()), frozen("PurgeReport"));
    assert_eq!(
        keys_of(&TranscriptTurn {
            role: "user".to_string(),
            content: "hello".to_string(),
            timestamp: None,
        }),
        frozen("TranscriptTurn")
    );
}
