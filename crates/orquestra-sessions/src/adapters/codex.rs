//! OpenAI Codex: `thread_history_1.sqlite` for turn statistics,
//! `session_index.jsonl` for thread names, and
//! `sessions/YYYY/MM/DD/rollout-*.jsonl` for the bytes and the working
//! directory.
//!
//! The three sources disagree about which threads exist — a rollout can
//! outlive its history rows and the index can name a thread with no turns left
//! — so the union is listed and each field falls back in that order.
//!
//! [`indexes`] reads the two indexes, [`files`] owns the rollout tree and the
//! row deletes, and this module assembles the listing.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::types::{TranscriptTurn, UnifiedSession};

use super::{fsops, transcript, utils, SessionAdapter};
use files::{
    delete_rows_in, maintenance_lock_held, project_path, prune_empty_parents, rollout_files, title,
};

mod files;
mod indexes;

const HARNESS: &str = "codex";
const STORE_REL: &str = ".codex";
const DATABASE: &str = "thread_history_1.sqlite";
/// Codex runs its own maintenance pass under this lock; deleting rows while it
/// does is how a history database gets corrupted.
const MAINTENANCE_LOCK: &str = ".sqlite-maintenance.lock";
/// Log rows carry the thread id in this database.
const LOG_DATABASE: &str = "logs_2.sqlite";
/// Tables holding thread rows in [`DATABASE`].
const THREAD_TABLES: [(&str, &str); 4] = [
    ("thread_items", "thread_id"),
    ("thread_turns", "thread_id"),
    ("thread_history_projection_state", "thread_id"),
    ("thread_realtime_items", "thread_id"),
];
/// Rollout files end in `-<uuid>.jsonl`.
const THREAD_ID_LEN: usize = 36;

pub struct Codex {
    root: PathBuf,
}

impl Codex {
    pub fn new(home: &Path) -> Self {
        Self {
            root: home.to_path_buf(),
        }
    }

    fn store(&self) -> PathBuf {
        self.root.join(STORE_REL)
    }

    /// Rollout files named after this thread id, anywhere under `sessions/`.
    fn rollouts_for(&self, session_id: &str) -> Vec<PathBuf> {
        let suffix = format!("-{session_id}.jsonl");

        rollout_files(&self.store().join("sessions"))
            .into_values()
            .filter(|(path, _)| {
                path.to_string_lossy()
                    .rsplit('/')
                    .next()
                    .is_some_and(|name| name.ends_with(&suffix))
            })
            .map(|(path, _)| path)
            .collect()
    }
}

impl SessionAdapter for Codex {
    fn harness_id(&self) -> &'static str {
        HARNESS
    }

    fn is_available(&self) -> bool {
        self.store().is_dir()
    }

    fn list_sessions(&self) -> Result<Vec<UnifiedSession>, String> {
        let named = self.named_threads();
        let stats = self.turn_stats()?;
        let rollouts = rollout_files(&self.store().join("sessions"));

        let ids: BTreeSet<String> = named
            .keys()
            .chain(stats.keys())
            .chain(rollouts.keys())
            .cloned()
            .collect();

        let mut sessions = Vec::with_capacity(ids.len());
        for id in ids {
            let (name, index_updated) = match named.get(&id) {
                Some((name, updated)) => (name.clone(), *updated),
                None => (None, None),
            };
            let (created, updated, turns) = stats.get(&id).copied().unwrap_or((None, None, 0));
            let rollout = rollouts.get(&id);

            let mut session = UnifiedSession::new(HARNESS, id.clone(), title(&id, name));
            session.turns_count = turns;
            session.size_bytes = rollout.map(|(_, size)| *size).unwrap_or(0);
            session.project_path = rollout.and_then(|(path, _)| project_path(path));
            session.created_at = created.map(utils::iso8601_from_secs);
            session.updated_at = updated
                .or(index_updated)
                .or_else(|| rollout.and_then(|(path, _)| utils::modified_secs(path)))
                .map(utils::iso8601_from_secs)
                .unwrap_or_default();

            sessions.push(session);
        }

        Ok(sessions)
    }

    fn delete_session(&self, session_id: &str) -> Result<u64, String> {
        let store = self.store();
        if maintenance_lock_held(&store) {
            return Err(format!(
                "Codex is running a database maintenance pass ({MAINTENANCE_LOCK})"
            ));
        }

        let mut bytes = 0;
        for rollout in self.rollouts_for(session_id) {
            bytes += fsops::remove_path(&store, &rollout)?;
            prune_empty_parents(&store, &rollout);
        }

        // The CLI reads this index at startup: a row left behind by a deleted
        // rollout is what makes Codex list a thread that no longer exists.
        fsops::rewrite_jsonl(&store, &store.join("session_index.jsonl"), &|record| {
            record.get("id").and_then(|id| id.as_str()) != Some(session_id)
        })?;

        for (table, column) in THREAD_TABLES {
            delete_rows_in(&store.join(DATABASE), table, column, session_id)?;
        }
        delete_rows_in(&store.join(LOG_DATABASE), "logs", "thread_id", session_id)?;

        Ok(bytes)
    }

    /// Conversation turns from this thread's rollout files. Only
    /// `response_item` lines with a `user` or `assistant` payload role become
    /// turns: `developer` instructions, environment setup blocks and bare
    /// tool items are harness scaffolding, not conversation.
    fn read_transcript(&self, session_id: &str) -> Vec<TranscriptTurn> {
        if session_id.is_empty() || session_id.contains('/') || session_id.contains('\\') {
            return Vec::new();
        }

        // Streamed rollout-by-rollout with early stop: one line alive at a
        // time, and parsing halts at the turn cap instead of draining a
        // tens-of-megabytes file that the drawer will never display.
        let mut turns = Vec::new();
        for rollout in self.rollouts_for(session_id) {
            utils::for_each_jsonl(&rollout, &mut |entry| {
                if entry.get("type").and_then(Value::as_str) != Some("response_item") {
                    return true;
                }
                let Some(payload) = entry.get("payload") else {
                    return true;
                };
                if !matches!(
                    payload.get("role").and_then(Value::as_str),
                    Some("user") | Some("assistant")
                ) {
                    return true;
                }
                let normalized = json!({
                    "role": payload.get("role"),
                    "message": payload.get("content"),
                    "timestamp": entry.get("timestamp"),
                });
                if let Some(turn) = transcript::parse_value(&normalized) {
                    turns.push(turn);
                }
                turns.len() < transcript::MAX_TRANSCRIPT_TURNS
            });
            if turns.len() >= transcript::MAX_TRANSCRIPT_TURNS {
                break;
            }
        }
        turns
    }

    fn optimize_storage(&self) -> Result<(), String> {
        let store = self.store();
        if maintenance_lock_held(&store) {
            return Err(format!(
                "Codex is running a database maintenance pass ({MAINTENANCE_LOCK})"
            ));
        }

        // Hundreds of megabytes live in these two files and only VACUUM frees
        // the pages that a row delete leaves behind.
        for database in [DATABASE, LOG_DATABASE] {
            fsops::vacuum(&store.join(database))?;
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "codex_tests.rs"]
mod tests;
