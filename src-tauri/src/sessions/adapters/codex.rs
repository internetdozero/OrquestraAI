//! OpenAI Codex: `thread_history_1.sqlite` for turn statistics,
//! `session_index.jsonl` for thread names, and
//! `sessions/YYYY/MM/DD/rollout-*.jsonl` for the bytes and the working
//! directory.
//!
//! The three sources disagree about which threads exist — a rollout can
//! outlive its history rows and the index can name a thread with no turns left
//! — so the union is listed and each field falls back in that order.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::sessions::types::{TranscriptTurn, UnifiedSession};

use super::{fsops, transcript, utils, SessionAdapter};

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

/// `(created, updated, turns)` from the history database.
type TurnStats = (Option<i64>, Option<i64>, usize);

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

    /// Thread names and index timestamps, keyed by thread id.
    fn named_threads(&self) -> HashMap<String, (Option<String>, Option<i64>)> {
        utils::read_jsonl(&self.store().join("session_index.jsonl"))
            .into_iter()
            .filter_map(|entry| {
                let id = entry.get("id").and_then(|id| id.as_str())?.to_string();
                let name = utils::json_string(&entry, &["thread_name", "title"]);
                let updated = utils::json_timestamp(&entry, &["updated_at"]);
                Some((id, (name, updated)))
            })
            .collect()
    }

    /// Turn counts and activity window from the read-only history database.
    /// A missing database is normal; a corrupt one fails this harness only.
    fn turn_stats(&self) -> Result<HashMap<String, TurnStats>, String> {
        let database = self.store().join(DATABASE);
        if !database.is_file() {
            return Ok(HashMap::new());
        }

        let connection = utils::open_readonly(&database)?;
        if !utils::table_exists(&connection, "thread_turns")? {
            return Ok(HashMap::new());
        }

        let mut statement = connection
            .prepare(
                "SELECT thread_id,
                        MIN(COALESCE(started_at, completed_at)),
                        MAX(COALESCE(completed_at, started_at)),
                        COUNT(*)
                 FROM thread_turns
                 GROUP BY thread_id",
            )
            .map_err(|error| format!("{}: {error}", database.display()))?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    utils::text_cell(row, 0),
                    utils::i64_cell(row, 1),
                    utils::i64_cell(row, 2),
                    utils::i64_cell(row, 3),
                ))
            })
            .map_err(|error| format!("{}: {error}", database.display()))?;

        Ok(rows
            .flatten()
            .filter_map(|(id, created, updated, turns)| {
                Some((id?, (created, updated, turns.unwrap_or(0).max(0) as usize)))
            })
            .collect())
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

        let mut turns = Vec::new();
        for rollout in self.rollouts_for(session_id) {
            for entry in utils::read_jsonl(&rollout) {
                if entry.get("type").and_then(Value::as_str) != Some("response_item") {
                    continue;
                }
                let Some(payload) = entry.get("payload") else {
                    continue;
                };
                if !matches!(
                    payload.get("role").and_then(Value::as_str),
                    Some("user") | Some("assistant")
                ) {
                    continue;
                }
                let normalized = json!({
                    "role": payload.get("role"),
                    "message": payload.get("content"),
                    "timestamp": entry.get("timestamp"),
                });
                if let Some(turn) = transcript::parse_value(&normalized) {
                    turns.push(turn);
                    if turns.len() >= transcript::MAX_TRANSCRIPT_TURNS {
                        break;
                    }
                }
            }
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

/// `thread_id -> (path, bytes)` for every rollout file.
fn rollout_files(sessions_root: &Path) -> HashMap<String, (PathBuf, u64)> {
    let mut found = HashMap::new();
    let mut stack = vec![sessions_root.to_path_buf()];

    while let Some(current) = stack.pop() {
        // `read_dir` follows a symlink. `sessions/` replaced by a link would
        // list and then delete rollouts outside the store.
        let real_dir = std::fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_dir())
            .unwrap_or(false);
        if !real_dir {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };

        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };

            if kind.is_dir() {
                stack.push(entry.path());
                continue;
            }

            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(target) = name
                .strip_prefix("rollout-")
                .and_then(|rest| rest.strip_suffix(".jsonl"))
            else {
                continue;
            };
            let id = target
                .get(target.len().saturating_sub(THREAD_ID_LEN)..)
                .unwrap_or(target)
                .to_string();
            let size = utils::file_size(&entry.path());

            found.insert(id, (entry.path(), size));
        }
    }

    found
}

/// Delete rows from one table, tolerating a database this release does not
/// have. A missing file is not a failure: a harness may keep only rollouts.
fn delete_rows_in(
    database: &Path,
    table: &str,
    column: &str,
    session_id: &str,
) -> Result<(), String> {
    if !database.is_file() {
        return Ok(());
    }

    let connection = fsops::open_writable(database)?;
    fsops::delete_rows(&connection, table, column, session_id)?;
    Ok(())
}

/// Remove the day/month folders a rollout leaves behind once empty.
fn prune_empty_parents(store: &Path, file: &Path) {
    let mut current = file.parent().map(Path::to_path_buf);

    while let Some(directory) = current {
        if !fsops::remove_if_empty(store, &directory) {
            break;
        }
        current = directory.parent().map(Path::to_path_buf);
    }
}

/// The rollout header records the working directory the thread ran in.
fn project_path(rollout: &Path) -> Option<String> {
    let header = utils::first_json_line(rollout)?;
    let payload = header.get("payload").unwrap_or(&header);
    utils::json_string(payload, &["cwd", "working_directory", "workdir"])
}

/// A real lock file means Codex is inside a maintenance pass. A symlink is
/// not that file: `exists` would follow it and refuse every delete.
fn maintenance_lock_held(store: &Path) -> bool {
    std::fs::symlink_metadata(store.join(MAINTENANCE_LOCK))
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn title(id: &str, name: Option<String>) -> String {
    match name.map(|name| utils::trim_title(&name)) {
        Some(name) if !name.is_empty() => name,
        _ => format!("Thread {}", id.chars().take(8).collect::<String>()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[path = "codex_tests.rs"]
mod tests;
