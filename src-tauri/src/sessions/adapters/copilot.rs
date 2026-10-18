//! GitHub Copilot CLI: `session-store.db` for the session rows and turns, and
//! `session-state/<id>/` for the per-session workspace.
//!
//! Lock files under `session-state/.session-operation-locks/` are only read
//! here — listing never touches a session that a running CLI holds open.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::sessions::types::{TranscriptTurn, UnifiedSession};

use super::{fsops, transcript, utils, SessionAdapter};

const HARNESS: &str = "copilot";
const STORE_REL: &str = ".copilot";
const DATABASE: &str = "session-store.db";
/// Per-session locks the CLI takes while it is working on a conversation.
const LOCKS_REL: &str = "session-state/.session-operation-locks";
/// Registry of conversations the CLI currently considers open.
const OPEN_STATE: &str = "open-sessions-state.json";

/// `session_id -> (turns, content bytes)`.
type TurnSummary = (usize, u64);

pub struct Copilot {
    root: PathBuf,
}

impl Copilot {
    pub fn new(home: &Path) -> Self {
        Self {
            root: home.to_path_buf(),
        }
    }

    fn store(&self) -> PathBuf {
        self.root.join(STORE_REL)
    }

    /// Turn counts and message bytes per session, read-only.
    fn turn_summary(&self) -> Result<HashMap<String, TurnSummary>, String> {
        let database = self.store().join(DATABASE);
        if !database.is_file() {
            return Ok(HashMap::new());
        }

        let connection = utils::open_readonly(&database)?;
        if !utils::table_exists(&connection, "turns")? {
            return Ok(HashMap::new());
        }

        let mut statement = connection
            .prepare(
                "SELECT session_id,
                        COUNT(*),
                        SUM(LENGTH(COALESCE(user_message, ''))
                            + LENGTH(COALESCE(assistant_response, '')))
                 FROM turns
                 GROUP BY session_id",
            )
            .map_err(|error| format!("{}: {error}", database.display()))?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    utils::text_cell(row, 0),
                    utils::i64_cell(row, 1),
                    utils::i64_cell(row, 2),
                ))
            })
            .map_err(|error| format!("{}: {error}", database.display()))?;

        Ok(rows
            .flatten()
            .filter_map(|(id, turns, bytes)| {
                Some((
                    id?,
                    (
                        turns.unwrap_or(0).max(0) as usize,
                        bytes.unwrap_or(0).max(0) as u64,
                    ),
                ))
            })
            .collect())
    }

    fn database_sessions(&self) -> Result<Vec<UnifiedSession>, String> {
        let database = self.store().join(DATABASE);
        if !database.is_file() {
            return Ok(Vec::new());
        }

        let connection = utils::open_readonly(&database)?;
        if !utils::table_exists(&connection, "sessions")? {
            return Ok(Vec::new());
        }

        let mut statement = connection
            .prepare("SELECT id, cwd, repository, summary, created_at, updated_at FROM sessions")
            .map_err(|error| format!("{}: {error}", database.display()))?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    utils::text_cell(row, 0),
                    utils::text_cell(row, 1),
                    utils::text_cell(row, 2),
                    utils::text_cell(row, 3),
                    utils::text_cell(row, 4),
                    utils::text_cell(row, 5),
                ))
            })
            .map_err(|error| format!("{}: {error}", database.display()))?;

        let turn_summary = self.turn_summary()?;

        Ok(rows
            .flatten()
            .filter_map(|(id, cwd, repository, summary, created, updated)| {
                let id = id?;
                if id.is_empty() {
                    return None;
                }

                let (turns, content_bytes) = turn_summary.get(&id).copied().unwrap_or((0, 0));
                let state = self.store().join("session-state").join(&id);

                let mut session = UnifiedSession::new(HARNESS, id.clone(), title(&id, summary));
                session.project_path = cwd
                    .filter(|path| !path.trim().is_empty())
                    .or_else(|| repository.filter(|repo| !repo.trim().is_empty()));
                session.created_at = created.as_deref().map(utils::normalized_timestamp);
                session.updated_at = updated
                    .or(created)
                    .as_deref()
                    .map(utils::normalized_timestamp)
                    .unwrap_or_default();
                session.turns_count = turns;
                session.size_bytes = utils::dir_size(&state) + content_bytes;

                Some(session)
            })
            .collect())
    }

    /// Sessions whose state folder survived a database wipe.
    fn orphan_state(&self) -> Vec<UnifiedSession> {
        let Ok(entries) = std::fs::read_dir(self.store().join("session-state")) else {
            return Vec::new();
        };

        entries
            .flatten()
            .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
            .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
            .map(|entry| {
                let id = entry.file_name().to_string_lossy().into_owned();
                let path = entry.path();
                let mut session = UnifiedSession::new(HARNESS, id.clone(), title(&id, None));
                session.updated_at = utils::modified_secs(&path)
                    .map(utils::iso8601_from_secs)
                    .unwrap_or_default();
                session.size_bytes = utils::dir_size(&path);
                session
            })
            .collect()
    }
}

impl SessionAdapter for Copilot {
    fn harness_id(&self) -> &'static str {
        HARNESS
    }

    fn is_available(&self) -> bool {
        self.store().is_dir()
    }

    fn is_session_active(&self, session_id: &str) -> bool {
        self.store()
            .join(LOCKS_REL)
            .join(format!("{session_id}.lock"))
            .exists()
    }

    fn delete_session(&self, session_id: &str) -> Result<u64, String> {
        let store = self.store();
        let mut bytes = 0;

        let database = store.join(DATABASE);
        if database.is_file() {
            let connection = fsops::open_writable(&database)?;
            // Turns first: `sessions` is the parent the child rows point at.
            fsops::delete_rows(&connection, "turns", "session_id", session_id)?;
            fsops::delete_rows(&connection, "sessions", "id", session_id)?;
        }

        bytes += fsops::remove_path(&store, &store.join("session-state").join(session_id))?;
        fsops::remove_json_key(&store, &store.join(OPEN_STATE), session_id)?;

        Ok(bytes)
    }

    fn optimize_storage(&self) -> Result<(), String> {
        fsops::vacuum(&self.store().join(DATABASE))
    }

    /// Conversation turns from the `turns` table: one user turn plus one
    /// assistant turn per row, in `turn_index` order. Empty halves are
    /// skipped, so a turn still waiting for its response shows just the
    /// prompt. A missing table or database is an empty feed.
    fn read_transcript(&self, session_id: &str) -> Vec<TranscriptTurn> {
        let database = self.store().join(DATABASE);
        if !database.is_file() {
            return Vec::new();
        }

        let Ok(connection) = utils::open_readonly(&database) else {
            return Vec::new();
        };
        if !utils::table_exists(&connection, "turns").unwrap_or(false) {
            return Vec::new();
        }

        let columns = utils::table_columns(&connection, "turns");
        for required in ["session_id", "user_message", "assistant_response"] {
            if !columns.contains(&required.to_string()) {
                return Vec::new();
            }
        }
        let order = if columns.contains(&"turn_index".to_string()) {
            "ORDER BY turn_index ASC"
        } else {
            "ORDER BY rowid ASC"
        };
        let timestamp = if columns.contains(&"timestamp".to_string()) {
            "timestamp"
        } else {
            "NULL"
        };

        let Ok(mut statement) = connection.prepare(&format!(
            "SELECT user_message, assistant_response, {timestamp} \
             FROM turns WHERE session_id = ?1 {order}"
        )) else {
            return Vec::new();
        };
        let Ok(rows) = statement.query_map([session_id], |row| {
            Ok((
                utils::text_cell(row, 0),
                utils::text_cell(row, 1),
                utils::text_cell(row, 2),
            ))
        }) else {
            return Vec::new();
        };

        let mut turns = Vec::new();
        for row in rows.flatten() {
            let (user, assistant, timestamp) = row;
            let stamped = timestamp
                .filter(|stamp| !stamp.trim().is_empty())
                .map(|stamp| utils::normalized_timestamp(&stamp));
            for (role, content) in [
                ("user", user.as_deref().unwrap_or("")),
                ("assistant", assistant.as_deref().unwrap_or("")),
            ] {
                if content.trim().is_empty() {
                    continue;
                }
                if let Some(turn) =
                    transcript::build_turn(role, content, stamped.clone())
                {
                    turns.push(turn);
                    if turns.len() >= transcript::MAX_TRANSCRIPT_TURNS {
                        return turns;
                    }
                }
            }
        }
        turns
    }

    fn list_sessions(&self) -> Result<Vec<UnifiedSession>, String> {
        let mut sessions = self.database_sessions()?;
        let known: HashSet<String> = sessions.iter().map(|session| session.id.clone()).collect();

        for session in self.orphan_state() {
            if !known.contains(&session.id) {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }
}

fn title(id: &str, summary: Option<String>) -> String {
    match summary.map(|summary| utils::trim_title(&summary)) {
        Some(summary) if !summary.is_empty() => summary,
        _ => format!("Session {}", id.chars().take(8).collect::<String>()),
    }
}
