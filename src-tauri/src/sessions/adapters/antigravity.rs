//! Google Antigravity: a summary index plus one artifact folder per
//! conversation.
//!
//! `conversation_summaries.db` carries the titles and step counts; the bytes a
//! conversation costs are in `brain/<id>/` and `conversations/<id>.db`. The
//! CLI variant ships the same layout under `antigravity-cli`, so both are read
//! and duplicate ids are collapsed.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::sessions::types::{TranscriptTurn, UnifiedSession};

use super::{fsops, transcript, utils, SessionAdapter};

const HARNESS: &str = "antigravity";
const STORES: [&str; 2] = [".gemini/antigravity", ".gemini/antigravity-cli"];
const SUMMARY_DATABASE: &str = "conversation_summaries.db";

pub struct Antigravity {
    root: PathBuf,
}

impl Antigravity {
    pub fn new(home: &Path) -> Self {
        Self {
            root: home.to_path_buf(),
        }
    }

    fn stores(&self) -> Vec<PathBuf> {
        STORES
            .iter()
            .map(|relative| self.root.join(relative))
            .filter(|path| path.is_dir())
            .collect()
    }
}

impl SessionAdapter for Antigravity {
    fn harness_id(&self) -> &'static str {
        HARNESS
    }

    fn is_available(&self) -> bool {
        !self.stores().is_empty()
    }

    fn list_sessions(&self) -> Result<Vec<UnifiedSession>, String> {
        let mut seen = HashSet::new();
        let mut sessions = Vec::new();

        for store in self.stores() {
            for session in list_store(&store)? {
                if seen.insert(session.id.clone()) {
                    sessions.push(session);
                }
            }
        }

        Ok(sessions)
    }

    fn delete_session(&self, session_id: &str) -> Result<u64, String> {
        // `brain/<id>` and `conversations/<id>.db` are path joins. A directory
        // symlink named `escape` plus id `escape/secret` deletes the outside
        // child; `read_transcript` already refuses that id.
        if !single_segment(session_id) {
            return Ok(0);
        }
        let mut bytes = 0;

        for store in self.stores() {
            // Artifacts first: a brain folder with no summary row is what shows
            // up as an orphan in Antigravity's sidebar, so it goes too.
            let brain_root = store.join("brain");
            let brain = brain_root.join(session_id);
            // `brain/` as a directory symlink makes `<id>` a real directory,
            // and `remove_path` would delete that folder outside the store.
            if real_dir(&brain_root) && std::fs::symlink_metadata(&brain).is_ok() {
                bytes += fsops::remove_path(&store, &brain)?;
            }

            let conversations = store.join("conversations");
            let conversation = conversations.join(format!("{session_id}.db"));
            // `conversations/` as a directory symlink makes `<id>.db` a regular
            // file, and `remove_sqlite` would delete it outside the store.
            if real_dir(&conversations) {
                bytes += fsops::remove_sqlite(&store, &conversation)?;
            }

            let database = store.join(SUMMARY_DATABASE);
            if database.is_file() {
                let connection = fsops::open_writable(&database)?;
                fsops::delete_rows(
                    &connection,
                    "conversation_summaries",
                    "conversation_id",
                    session_id,
                )?;
            }
        }

        Ok(bytes)
    }

    fn optimize_storage(&self) -> Result<(), String> {
        for store in self.stores() {
            fsops::vacuum(&store.join(SUMMARY_DATABASE))?;
        }

        Ok(())
    }

    /// Every conversation turn in `brain/<id>/…/logs/transcript.jsonl`,
    /// checked in both the desktop and CLI stores. Either layout may or may
    /// not nest the logs under `.system_generated`; a missing transcript is
    /// an empty feed.
    fn read_transcript(&self, session_id: &str) -> Vec<TranscriptTurn> {
        // `brain/<id>` is joined as a path. `../` still stats as a real
        // directory after the kernel resolves it, so the id has to be one
        // segment before any transcript is opened.
        if session_id.is_empty()
            || session_id.contains('/')
            || session_id.contains('\\')
            || session_id == "."
            || session_id == ".."
        {
            return Vec::new();
        }
        for relative in STORES {
            let brain_root = self.root.join(relative).join("brain");
            if !real_dir(&brain_root) {
                continue;
            }
            let brain = brain_root.join(session_id);
            // `brain/<id>` as a directory symlink makes `transcript.jsonl` a
            // regular file, and the read would load it from outside the store.
            if !real_dir(&brain) {
                continue;
            }
            for candidate in [
                brain
                    .join(".system_generated")
                    .join("logs")
                    .join("transcript.jsonl"),
                brain.join("logs").join("transcript.jsonl"),
            ] {
                if candidate.is_file() {
                    return transcript::read_transcript(&candidate);
                }
            }
        }

        Vec::new()
    }
}

fn list_store(store: &Path) -> Result<Vec<UnifiedSession>, String> {
    let database = store.join(SUMMARY_DATABASE);
    if !database.is_file() {
        return Ok(fallback_from_brain(store));
    }

    let connection = utils::open_readonly(&database)?;
    if !utils::table_exists(&connection, "conversation_summaries")? {
        return Ok(fallback_from_brain(store));
    }

    let mut statement = connection
        .prepare(
            "SELECT conversation_id, title, preview, step_count,
                    last_modified_time, workspace_uris
             FROM conversation_summaries",
        )
        .map_err(|error| format!("{}: {error}", database.display()))?;

    let rows = statement
        .query_map([], |row| {
            Ok((
                utils::text_cell(row, 0),
                utils::text_cell(row, 1),
                utils::text_cell(row, 2),
                utils::i64_cell(row, 3),
                utils::text_cell(row, 4),
                utils::text_cell(row, 5),
            ))
        })
        .map_err(|error| format!("{}: {error}", database.display()))?;

    let mut sessions = Vec::new();
    for row in rows.flatten() {
        let (Some(id), title, preview, steps, modified, workspaces) = row else {
            continue;
        };
        if id.is_empty() {
            continue;
        }

        let mut session = UnifiedSession::new(HARNESS, id.clone(), label(&id, title, preview));
        session.turns_count = steps.unwrap_or(0).max(0) as usize;
        session.updated_at = modified
            .as_deref()
            .map(utils::normalized_timestamp)
            .unwrap_or_default();
        session.project_path = workspaces.as_deref().and_then(workspace_path);
        session.size_bytes = artifact_size(store, &id);

        sessions.push(session);
    }

    Ok(sessions)
}

/// Conversations whose summary row is gone but whose artifacts remain: without
/// this the tab would hide bytes that are still on disk.
fn fallback_from_brain(store: &Path) -> Vec<UnifiedSession> {
    let brain = store.join("brain");
    if !real_dir(&brain) {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&brain) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            let mut session = UnifiedSession::new(HARNESS, id.clone(), label(&id, None, None));
            session.updated_at = utils::modified_secs(&entry.path())
                .map(utils::iso8601_from_secs)
                .unwrap_or_default();
            session.size_bytes = artifact_size(store, &id);
            session
        })
        .collect()
}

/// `brain/<id>/` plus `conversations/<id>.db`, excluding the shared databases.
fn artifact_size(store: &Path, id: &str) -> u64 {
    // `brain/<id>` with `../outside` still names a real directory after the
    // kernel resolves it, and `dir_size` would walk that tree.
    if id.is_empty() || id.contains('/') || id.contains('\\') || id == "." || id == ".." {
        return 0;
    }
    let brain = store.join("brain");
    let folder = if real_dir(&brain) {
        utils::dir_size(&brain.join(id))
    } else {
        0
    };
    let conversations = store.join("conversations");
    let database = if real_dir(&conversations) {
        utils::file_size(&conversations.join(format!("{id}.db")))
    } else {
        0
    };
    folder + database
}

fn single_segment(id: &str) -> bool {
    !id.is_empty() && !id.contains('/') && !id.contains('\\') && id != "." && id != ".."
}

fn real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

/// `workspace_uris` is a JSON array of `file:///path/` entries; the tab shows
/// the first one as the project.
fn workspace_path(raw: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(raw).ok()?;
    let first = parsed.as_array()?.first()?.as_str()?.trim();
    let path = first.strip_prefix("file://").unwrap_or(first);
    let trimmed = path.trim_end_matches('/');

    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn label(id: &str, title: Option<String>, preview: Option<String>) -> String {
    let text = title
        .into_iter()
        .chain(preview)
        .find(|text| !text.trim().is_empty())
        .map(|text| utils::trim_title(&text))
        .unwrap_or_default();

    if text.is_empty() {
        format!("Conversation {}", id.chars().take(8).collect::<String>())
    } else {
        text
    }
}
