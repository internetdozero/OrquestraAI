//! xAI Grok: `sessions/<encoded-cwd>/<id>/` folders indexed by
//! `sessions/session_search.sqlite` (`session_docs`).
//!
//! The index and the folders drift apart — a deleted folder can leave its
//! document row behind — so both are listed and merged by session id.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::sessions::types::{TranscriptTurn, UnifiedSession};

use super::{fsops, transcript, utils, SessionAdapter};

const HARNESS: &str = "grok";
const STORE_REL: &str = ".grok/sessions";
const DATABASE: &str = "session_search.sqlite";
/// Grok publishes the conversations it is holding here; deleting one of those
/// under a running session is exactly what corrupts it.
const ACTIVE_STATE: &str = ".grok/active_sessions.json";

/// One indexed document: title, recorded cwd and update time.
struct Indexed {
    title: Option<String>,
    cwd: Option<String>,
    updated: Option<i64>,
}

pub struct Grok {
    root: PathBuf,
}

impl Grok {
    pub fn new(home: &Path) -> Self {
        Self {
            root: home.to_path_buf(),
        }
    }

    fn store(&self) -> PathBuf {
        self.root.join(STORE_REL)
    }

    fn indexed(&self) -> Result<HashMap<String, Indexed>, String> {
        let database = self.store().join(DATABASE);
        if !database.is_file() {
            return Ok(HashMap::new());
        }

        let connection = utils::open_readonly(&database)?;
        if !utils::table_exists(&connection, "session_docs")? {
            return Ok(HashMap::new());
        }

        let mut statement = connection
            .prepare("SELECT session_id, title, cwd, updated_at FROM session_docs")
            .map_err(|error| format!("{}: {error}", database.display()))?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    utils::text_cell(row, 0),
                    utils::text_cell(row, 1),
                    utils::text_cell(row, 2),
                    utils::text_cell(row, 3),
                ))
            })
            .map_err(|error| format!("{}: {error}", database.display()))?;

        Ok(rows
            .flatten()
            .filter_map(|(id, title, cwd, updated)| {
                Some((
                    id?,
                    Indexed {
                        title: title.filter(|title| !title.trim().is_empty()),
                        cwd: cwd.filter(|cwd| !cwd.trim().is_empty()),
                        updated: updated.as_deref().and_then(utils::timestamp_secs),
                    },
                ))
            })
            .collect())
    }

    /// Session ids Grok currently has open. The file holds either bare ids or
    /// objects with a `session_id` / `id` field, depending on release.
    fn active_ids(&self) -> Vec<String> {
        let Some(value) = utils::read_json(&self.root.join(ACTIVE_STATE)) else {
            return Vec::new();
        };
        let entries = match value {
            serde_json::Value::Array(entries) => entries,
            serde_json::Value::Object(map) => map
                .keys()
                .map(|key| serde_json::Value::String(key.clone()))
                .collect(),
            _ => Vec::new(),
        };

        entries
            .iter()
            .filter_map(|entry| match entry {
                serde_json::Value::String(id) => Some(id.clone()),
                other => utils::json_string(other, &["session_id", "id"]),
            })
            .collect()
    }

    /// `session_id -> (folder, bytes, encoded cwd)`.
    fn folders(&self) -> HashMap<String, (PathBuf, u64, String)> {
        let mut found = HashMap::new();
        let Ok(projects) = std::fs::read_dir(self.store()) else {
            return found;
        };

        for project in projects.flatten() {
            if !project
                .file_type()
                .map(|kind| kind.is_dir())
                .unwrap_or(false)
            {
                continue;
            }
            let encoded = project.file_name().to_string_lossy().into_owned();
            let Ok(entries) = std::fs::read_dir(project.path()) else {
                continue;
            };

            for entry in entries.flatten() {
                if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                    continue;
                }
                let path = entry.path();
                let id = entry.file_name().to_string_lossy().into_owned();
                let size = utils::dir_size(&path);
                found.insert(id, (path, size, encoded.clone()));
            }
        }

        found
    }
}

impl SessionAdapter for Grok {
    fn harness_id(&self) -> &'static str {
        HARNESS
    }

    fn is_available(&self) -> bool {
        self.store().is_dir()
    }

    fn is_session_active(&self, session_id: &str) -> bool {
        self.active_ids().iter().any(|id| id == session_id)
    }

    fn delete_session(&self, session_id: &str) -> Result<u64, String> {
        let store = self.store();
        let mut bytes = 0;

        // The `session_docs` triggers keep `session_docs_fts` in step, so the
        // search index needs no separate cleanup here.
        let database = store.join(DATABASE);
        if database.is_file() {
            let connection = fsops::open_writable(&database)?;
            fsops::delete_rows(&connection, "session_docs", "session_id", session_id)?;
        }

        if let Ok(projects) = std::fs::read_dir(&store) {
            for project in projects.flatten() {
                if !project
                    .file_type()
                    .map(|kind| kind.is_dir())
                    .unwrap_or(false)
                {
                    continue;
                }

                let folder = project.path().join(session_id);
                if std::fs::symlink_metadata(&folder).is_ok() {
                    bytes += fsops::remove_path(&store, &folder)?;
                }
                fsops::remove_if_empty(&store, &project.path());
            }
        }

        Ok(bytes)
    }

    fn optimize_storage(&self) -> Result<(), String> {
        fsops::vacuum(&self.store().join(DATABASE))
    }

    /// Conversation turns in `sessions/<encoded-cwd>/<id>/chat_history.jsonl`.
    /// The folder layout mirrors [`Self::folders`]: every project directory
    /// is scanned for the session folder, so an encoded cwd the index forgot
    /// still resolves. A missing log is an empty feed, not an error.
    ///
    /// [`Self::folders`]: Grok::folders
    fn read_transcript(&self, session_id: &str) -> Vec<TranscriptTurn> {
        if session_id.is_empty()
            || session_id.contains('/')
            || session_id.contains('\\')
            || session_id == "."
            || session_id == ".."
        {
            return Vec::new();
        }
        let Ok(projects) = std::fs::read_dir(self.store()) else {
            return Vec::new();
        };

        for project in projects.flatten() {
            if !project
                .file_type()
                .map(|kind| kind.is_dir())
                .unwrap_or(false)
            {
                continue;
            }
            let log = project.path().join(session_id).join("chat_history.jsonl");
            if log.is_file() {
                return transcript::read_transcript(&log);
            }
        }

        Vec::new()
    }

    fn list_sessions(&self) -> Result<Vec<UnifiedSession>, String> {
        let indexed = self.indexed()?;
        let folders = self.folders();

        let ids: BTreeSet<String> = indexed.keys().chain(folders.keys()).cloned().collect();

        let mut sessions = Vec::with_capacity(ids.len());
        for id in ids {
            let index = indexed.get(&id);
            let folder = folders.get(&id);

            let mut session = UnifiedSession::new(
                HARNESS,
                id.clone(),
                title(&id, index.and_then(|index| index.title.clone())),
            );
            session.project_path = index
                .and_then(|index| index.cwd.clone())
                .or_else(|| folder.and_then(|(_, _, encoded)| utils::decode_path_hint(encoded)));
            session.size_bytes = folder.map(|(_, size, _)| *size).unwrap_or(0);
            session.updated_at = index
                .and_then(|index| index.updated)
                .or_else(|| folder.and_then(|(path, _, _)| utils::modified_secs(path)))
                .map(utils::iso8601_from_secs)
                .unwrap_or_default();

            sessions.push(session);
        }

        Ok(sessions)
    }
}

fn title(id: &str, indexed: Option<String>) -> String {
    match indexed.map(|title| utils::trim_title(&title)) {
        Some(title) if !title.is_empty() => title,
        _ => format!("Session {}", id.chars().take(8).collect::<String>()),
    }
}
