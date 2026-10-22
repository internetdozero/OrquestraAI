//! Unified session and chat-history management.
//!
//! Phase 1: a read-only catalog of every conversation the nine harnesses keep
//! on disk, plus the pin and retention sidecar in `~/.orquestra`. Listing never
//! writes to a harness store — a harness that is broken, locked or absent is
//! reported on stderr and skipped, so one bad database cannot empty the tab.
//!
//! Everything but the IPC surface lives in `orquestra-sessions`: the wire
//! types and the nine harness adapters, the pin and retention sidecar
//! (`policy`), purge execution (`purge`) and the auto-purge daemon
//! (`scheduler`). This module keeps only the commands the React view calls.

// Re-exported so every `crate::sessions::{adapters, types, purge}` path —
// including the `dir_size` bench — reads exactly as before.
pub use orquestra_sessions::{adapters, purge, types};

use std::collections::HashMap;

#[cfg(test)]
mod contract_tests;

use orquestra_sessions::policy::{self, pins};
use orquestra_sessions::scheduler;
use types::{
    PurgeEstimate, PurgeFilter, PurgeReport, RetentionPolicy, TranscriptTurn, UnifiedSession,
};

/// Every session from every installed harness, newest first, with pin state
/// applied from `~/.orquestra/sessions_policy.json`.
///
/// Runs on the async runtime rather than the main thread: sizing Freebuff
/// worktrees walks their `node_modules` folders and measures ~3.6 s on a real
/// machine, which would otherwise freeze the window mid-scan.
#[tauri::command(async)]
pub fn get_unified_sessions() -> Result<Vec<UnifiedSession>, String> {
    let mut sessions = adapters::list_all()?;
    let pins: HashMap<(String, String), Option<String>> = pins()?
        .into_iter()
        .map(|pin| ((pin.harness, pin.session_id), pin.label))
        .collect();

    for session in &mut sessions {
        let key = (session.harness.clone(), session.id.clone());
        if let Some(label) = pins.get(&key) {
            session.is_pinned = true;
            session.pin_label = label.clone();
        }
    }

    // ISO 8601 UTC sorts chronologically as text. Ids break ties so the order
    // a harness happened to return is never visible in the UI.
    sessions.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.harness.cmp(&right.harness))
            .then_with(|| left.id.cmp(&right.id))
    });

    Ok(sessions)
}

/// Conversation turns for the detail drawer's transcript feed.
///
/// Only `claude` and `antigravity` transcripts are parsed today; every other
/// harness resolves to an empty feed so the UI can show its in-development
/// note instead of an error. Runs async: transcripts run to megabytes and
/// must not freeze the window mid-read.
#[tauri::command(async)]
pub fn get_session_transcript(
    harness: String,
    session_id: String,
) -> Result<Vec<TranscriptTurn>, String> {
    let home = adapters::home()?;
    let turns = adapters::adapters_by_id(&home)
        .get(harness.as_str())
        .map(|adapter| adapter.read_transcript(&session_id))
        .unwrap_or_default();
    Ok(turns)
}

/// Pin or unpin one conversation. Returns the state after the toggle.
#[tauri::command]
pub fn toggle_pin_session(
    harness: String,
    session_id: String,
    label: Option<String>,
) -> Result<bool, String> {
    policy::toggle_pin(&harness, &session_id, label)
}

/// Current retention rules, with defaults on first run.
#[tauri::command]
pub fn get_retention_policy() -> Result<RetentionPolicy, String> {
    policy::get_policy()
}

/// Replace the retention rules; the pin list is preserved.
#[tauri::command]
pub fn save_retention_policy(policy: RetentionPolicy) -> Result<(), String> {
    policy::save_policy(policy)
}

/// Dry run: what `purge_sessions` would delete for this filter, in sessions
/// and bytes. Nothing on disk is touched.
#[tauri::command(async)]
pub fn estimate_purge(filter: PurgeFilter) -> Result<PurgeEstimate, String> {
    purge::estimate(&purge::Store::system()?, &filter)
}

/// Delete every session the filter matches. Pinned sessions are kept unless
/// the filter explicitly asks for them, and a session a running agent holds
/// open is always skipped and reported.
#[tauri::command(async)]
pub fn purge_sessions(filter: PurgeFilter) -> Result<PurgeReport, String> {
    purge::execute(&purge::Store::system()?, &filter)
}

/// Start the retention daemon. Called once from app setup; it lives until the
/// process exits.
pub fn start_scheduler() {
    scheduler::spawn();
}
