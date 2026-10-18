//! Freebuff / Manicode / Freebuff Desktop: worktrees, project chat logs and
//! native desktop threads.
//!
//! Worktrees are the disk bomb of this harness — each one is a project clone
//! with its own `node_modules` — so their size is measured with a full recursive
//! walk that counts every regular file and never follows a symlink. The
//! infrastructure symlinks under `~/.freebuff` stay untouched: listing only
//! reads. Desktop threads live in `freebuff_desktop`; this adapter dispatches
//! to it by session-id shape.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::sessions::types::{TranscriptTurn, UnifiedSession};

use super::{freebuff_desktop, fsops, transcript, utils, SessionAdapter};

const HARNESS: &str = "freebuff";
const WORKTREES_REL: &str = ".freebuff/worktrees";
const MANICODE_REL: &str = ".config/manicode/projects";

pub struct Freebuff {
    root: PathBuf,
}

impl Freebuff {
    pub fn new(home: &Path) -> Self {
        Self {
            root: home.to_path_buf(),
        }
    }
}

impl SessionAdapter for Freebuff {
    fn harness_id(&self) -> &'static str {
        HARNESS
    }

    fn is_available(&self) -> bool {
        self.root.join(WORKTREES_REL).is_dir()
            || self.root.join(MANICODE_REL).is_dir()
            || freebuff_desktop::is_available(&self.root)
    }

    fn list_sessions(&self) -> Result<Vec<UnifiedSession>, String> {
        let mut sessions = worktrees(&self.root.join(WORKTREES_REL));
        sessions.extend(manicode_chats(&self.root.join(MANICODE_REL)));
        sessions.extend(freebuff_desktop::list(&self.root));
        Ok(sessions)
    }

    fn delete_session(&self, session_id: &str) -> Result<u64, String> {
        // Desktop threads carry their project slug, so they dispatch before
        // the worktree / Manicode shapes below are even considered.
        if let Some((slug, thread)) = freebuff_desktop::parse_id(session_id) {
            return freebuff_desktop::delete(&self.root, &slug, &thread);
        }

        let mut bytes = 0;

        // A worktree id is a thread id, so it is a single path segment. The
        // chat id is `<project>/<chat>`; either form is resolved here rather
        // than trusting the caller to know which one it passed.
        let (project, chat) = match session_id.split_once('/') {
            Some((project, chat)) => (Some(project), Some(chat)),
            None => (None, None),
        };

        if project.is_none() {
            let worktrees = self.root.join(WORKTREES_REL);
            let worktree = worktrees.join(session_id);
            if std::fs::symlink_metadata(&worktree).is_ok() {
                // This is the big one: a worktree carries its own node_modules.
                bytes += fsops::remove_path(&worktrees, &worktree)?;
            }
        }

        if let (Some(project), Some(chat)) = (project, chat) {
            let projects = self.root.join(MANICODE_REL);
            let chat_dir = projects.join(project).join("chats").join(chat);
            bytes += fsops::remove_path(&projects, &chat_dir)?;
        }

        Ok(bytes)
    }

    /// Worktrees and Manicode chats keep no database; the desktop
    /// `desktop-v2.db` files are vacuumed to reclaim deleted threads.
    fn optimize_storage(&self) -> Result<(), String> {
        freebuff_desktop::vacuum_all(&self.root)
    }

    /// Conversation turns for Manicode chats and desktop threads. Manicode
    /// logs are JSON arrays read straight through the transcript parser;
    /// desktop threads are read from their project's `desktop-v2.db`. Bare
    /// worktree ids have no message store — only metadata — so they report
    /// an empty feed.
    fn read_transcript(&self, session_id: &str) -> Vec<TranscriptTurn> {
        if let Some((slug, thread)) = freebuff_desktop::parse_id(session_id) {
            return freebuff_desktop::transcript(&self.root, &slug, &thread);
        }

        let Some((project, chat)) = session_id.split_once('/') else {
            return Vec::new();
        };
        if !valid_segment(project) || !valid_segment(chat) {
            return Vec::new();
        }
        let log = self
            .root
            .join(MANICODE_REL)
            .join(project)
            .join("chats")
            .join(chat)
            .join("chat-messages.json");
        if !log.is_file() {
            return Vec::new();
        }
        transcript::read_transcript(&log)
    }
}

/// One session per worktree directory, titled from its `<project>.thread.json`.
fn worktrees(root: &Path) -> Vec<UnifiedSession> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|entry| {
            let path = entry.path();
            let id = entry.file_name().to_string_lossy().into_owned();
            let meta = thread_metadata(&path);

            let mut session = UnifiedSession::new(HARNESS, id.clone(), title(&id, meta.as_ref()));
            session.project_path = meta.as_ref().and_then(|meta| {
                utils::json_string(
                    meta,
                    &["cwd", "project_path", "repository", "repo", "project"],
                )
            });
            session.updated_at = meta
                .as_ref()
                .and_then(|meta| utils::json_timestamp(meta, &["updated_at", "updatedAt"]))
                .or_else(|| utils::modified_secs(&path))
                .map(utils::iso8601_from_secs)
                .unwrap_or_default();
            session.created_at = meta
                .as_ref()
                .and_then(|meta| utils::json_timestamp(meta, &["created_at", "createdAt"]))
                .map(utils::iso8601_from_secs);
            session.size_bytes = utils::dir_size(&path);

            session
        })
        .collect()
}

/// The `<project>.thread.json` inside a worktree, at its root or one project
/// level down.
fn thread_metadata(worktree: &Path) -> Option<Value> {
    let Ok(entries) = std::fs::read_dir(worktree) else {
        return None;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() && path.to_string_lossy().ends_with(".thread.json") {
            if let Some(meta) = utils::read_json(&path) {
                return Some(meta);
            }
            continue;
        }

        if path.is_dir() {
            if let Some(meta) = thread_metadata(&path) {
                return Some(meta);
            }
        }
    }

    None
}

/// `projects/<project>/chats/<chat>/chat-messages.json`, one session per chat.
fn manicode_chats(root: &Path) -> Vec<UnifiedSession> {
    let Ok(projects) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut sessions = Vec::new();
    for project in projects.flatten() {
        let project_name = project.file_name().to_string_lossy().into_owned();
        let Ok(chats) = std::fs::read_dir(project.path().join("chats")) else {
            continue;
        };

        for chat in chats.flatten() {
            let log = chat.path().join("chat-messages.json");
            if !log.is_file() {
                continue;
            }

            let chat_name = chat.file_name().to_string_lossy().into_owned();
            let id = format!("{project_name}/{chat_name}");
            let messages = utils::read_json(&log);
            let (title, user_turns) = match messages.as_ref() {
                Some(messages) => summarize(messages),
                None => (None, 0),
            };

            let mut session = UnifiedSession::new(
                HARNESS,
                id.clone(),
                title.unwrap_or_else(|| format!("Chat {}", chat_name)),
            );
            session.turns_count = user_turns;
            session.size_bytes = utils::file_size(&log);
            session.updated_at = utils::modified_secs(&log)
                .map(utils::iso8601_from_secs)
                .unwrap_or_default();

            sessions.push(session);
        }
    }

    sessions
}

/// First user prompt and user-turn count from a chat log.
fn summarize(messages: &Value) -> (Option<String>, usize) {
    let Some(entries) = messages.as_array() else {
        return (None, 0);
    };

    let mut title = None;
    let mut turns = 0;

    for entry in entries {
        let is_user = entry.get("role").and_then(|role| role.as_str()) == Some("user");
        if !is_user {
            continue;
        }

        turns += 1;
        if title.is_none() {
            title = utils::message_text(entry).map(|text| utils::trim_title(&text));
        }
    }

    (title, turns)
}

/// A session-id path segment: single, non-empty, and never a traversal.
fn valid_segment(segment: &str) -> bool {
    !(segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains('/')
        || segment.contains('\\'))
}

fn title(id: &str, meta: Option<&Value>) -> String {
    let text = meta
        .and_then(|meta| utils::json_string(meta, &["title", "name", "thread_name", "branch"]))
        .unwrap_or_default();

    if text.is_empty() {
        format!("Worktree {}", id.chars().take(8).collect::<String>())
    } else {
        utils::trim_title(&text)
    }
}
