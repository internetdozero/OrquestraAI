//! Hermes Agent: `state.db` for the session rows, `messages` for the bytes
//! those rows own, and `sessions/` for the on-disk log copies.
//!
//! `started_at` / `ended_at` are REAL epoch seconds and `message_count` is
//! authoritative, so no message scan is needed for the turn count.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::sessions::types::UnifiedSession;

use super::{fsops, utils, SessionAdapter};

const HARNESS: &str = "hermes";
const STORE_REL: &str = ".hermes";
const DATABASE: &str = "state.db";
/// Log folders that keep a copy of a session beside the database.
const LOG_DIRS: [&str; 2] = ["sessions", "terminal-sessions"];

pub struct Hermes {
    root: PathBuf,
}

impl Hermes {
    pub fn new(home: &Path) -> Self {
        Self {
            root: home.to_path_buf(),
        }
    }

    fn store(&self) -> PathBuf {
        self.root.join(STORE_REL)
    }

    /// Message bytes per session, so a row with no log file still reports a
    /// realistic footprint.
    fn message_bytes(
        &self,
        connection: &rusqlite::Connection,
    ) -> Result<HashMap<String, u64>, String> {
        if !utils::table_exists(connection, "messages")? {
            return Ok(HashMap::new());
        }

        let mut statement = connection
            .prepare(
                "SELECT session_id, SUM(LENGTH(COALESCE(content, '')))
                 FROM messages
                 GROUP BY session_id",
            )
            .map_err(|error| format!("messages: {error}"))?;

        let rows = statement
            .query_map([], |row| {
                Ok((utils::text_cell(row, 0), utils::i64_cell(row, 1)))
            })
            .map_err(|error| format!("messages: {error}"))?;

        Ok(rows
            .flatten()
            .filter_map(|(id, bytes)| Some((id?, bytes.unwrap_or(0).max(0) as u64)))
            .collect())
    }

    /// On-disk log copies kept beside the database.
    fn log_bytes(&self, id: &str) -> u64 {
        let sessions = self.store().join("sessions");
        let candidates = [
            sessions.join(format!("{id}.jsonl")),
            sessions.join(format!("{id}.json")),
            sessions.join(id),
        ];

        candidates
            .iter()
            .map(|path| {
                if path.is_dir() {
                    utils::dir_size(path)
                } else {
                    utils::file_size(path)
                }
            })
            .sum()
    }
}

impl SessionAdapter for Hermes {
    fn harness_id(&self) -> &'static str {
        HARNESS
    }

    fn is_available(&self) -> bool {
        self.store().is_dir()
    }

    fn delete_session(&self, session_id: &str) -> Result<u64, String> {
        let store = self.store();
        let mut bytes = 0;

        let database = store.join(DATABASE);
        if database.is_file() {
            let connection = fsops::open_writable(&database)?;
            // Messages first: deleting the parent session first would leave
            // the message rows unreachable but still occupying the file.
            fsops::delete_rows(&connection, "messages", "session_id", session_id)?;
            fsops::delete_rows(&connection, "sessions", "id", session_id)?;
        }

        for directory in LOG_DIRS {
            for name in [
                format!("{session_id}.jsonl"),
                format!("{session_id}.json"),
                session_id.to_string(),
            ] {
                let path = store.join(directory).join(name);
                if std::fs::symlink_metadata(&path).is_ok() {
                    bytes += fsops::remove_path(&store, &path)?;
                }
            }
        }

        Ok(bytes)
    }

    fn optimize_storage(&self) -> Result<(), String> {
        fsops::vacuum(&self.store().join(DATABASE))
    }

    fn list_sessions(&self) -> Result<Vec<UnifiedSession>, String> {
        let database = self.store().join(DATABASE);
        if !database.is_file() {
            return Ok(Vec::new());
        }

        let connection = utils::open_readonly(&database)?;
        if !utils::table_exists(&connection, "sessions")? {
            return Ok(Vec::new());
        }

        let bytes = self.message_bytes(&connection)?;

        let mut statement = connection
            .prepare(
                "SELECT id, source, display_name, started_at, ended_at, message_count
                 FROM sessions",
            )
            .map_err(|error| format!("{}: {error}", database.display()))?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    utils::text_cell(row, 0),
                    utils::text_cell(row, 1),
                    utils::text_cell(row, 2),
                    utils::text_cell(row, 3),
                    utils::text_cell(row, 4),
                    utils::i64_cell(row, 5),
                ))
            })
            .map_err(|error| format!("{}: {error}", database.display()))?;

        let mut sessions = Vec::new();
        for row in rows.flatten() {
            let (Some(id), source, display, started, ended, messages) = row else {
                continue;
            };
            if id.is_empty() {
                continue;
            }

            let started = started.as_deref().and_then(utils::timestamp_secs);
            let ended = ended.as_deref().and_then(utils::timestamp_secs);

            let mut session = UnifiedSession::new(HARNESS, id.clone(), title(&id, display, source));
            session.turns_count = messages.unwrap_or(0).max(0) as usize;
            session.created_at = started.map(utils::iso8601_from_secs);
            session.updated_at = ended
                .or(started)
                .map(utils::iso8601_from_secs)
                .unwrap_or_default();
            session.size_bytes = bytes.get(&id).copied().unwrap_or(0) + self.log_bytes(&id);

            sessions.push(session);
        }

        Ok(sessions)
    }
}

fn title(id: &str, display: Option<String>, source: Option<String>) -> String {
    let text = display
        .filter(|text| !text.trim().is_empty())
        .or_else(|| {
            source
                .filter(|text| !text.trim().is_empty())
                .map(|text| format!("{text} session"))
        })
        .unwrap_or_default();

    if text.is_empty() {
        format!("Session {}", id.chars().take(8).collect::<String>())
    } else {
        utils::trim_title(&text)
    }
}
