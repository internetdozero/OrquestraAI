//! Claude Code: one transcript per session under `projects/<slug>/`, plus a
//! `history.jsonl` of prompts used for titles and activity.
//!
//! Project folder names are encoded paths (`-home-system`), which are lossy
//! when a directory contains a dash. The transcript itself records `cwd`, so
//! that is preferred and the slug is only a last resort.

use std::collections::{BTreeSet, HashMap};
use std::io::BufRead;
use std::path::{Path, PathBuf};

use crate::sessions::types::{TranscriptTurn, UnifiedSession};

use super::{fsops, transcript, utils, SessionAdapter};

const HARNESS: &str = "claude";
const STORE_REL: &str = ".claude";
/// Enough lines to reach the first user prompt: transcripts open with a mode,
/// permission mode and atis latch before any message.
const TRANSCRIPT_SCAN_LINES: usize = 40;

/// Prompts recorded for one session in `history.jsonl`.
#[derive(Default)]
struct History {
    title: Option<String>,
    project: Option<String>,
    created: Option<i64>,
    updated: Option<i64>,
    prompts: usize,
}

pub struct Claude {
    root: PathBuf,
}

impl Claude {
    pub fn new(home: &Path) -> Self {
        Self {
            root: home.to_path_buf(),
        }
    }

    fn store(&self) -> PathBuf {
        self.root.join(STORE_REL)
    }

    fn history(&self) -> HashMap<String, History> {
        let mut history: HashMap<String, History> = HashMap::new();

        for entry in utils::read_jsonl(&self.store().join("history.jsonl")) {
            let Some(session) = entry.get("sessionId").and_then(|id| id.as_str()) else {
                continue;
            };

            let record = history.entry(session.to_string()).or_default();
            record.prompts += 1;

            let display = entry.get("display").and_then(|text| text.as_str());
            let useful = display.filter(|text| !text.trim().is_empty() && !text.starts_with('/'));
            if record.title.is_none() {
                record.title = useful.map(utils::trim_title);
            }
            if record.project.is_none() {
                record.project = utils::json_string(&entry, &["project"]);
            }
            if let Some(at) = utils::json_timestamp(&entry, &["timestamp", "created_at"]) {
                record.created = Some(record.created.map_or(at, |known: i64| known.min(at)));
                record.updated = Some(record.updated.map_or(at, |known: i64| known.max(at)));
            }
        }

        history
    }
}

impl SessionAdapter for Claude {
    fn harness_id(&self) -> &'static str {
        HARNESS
    }

    fn is_available(&self) -> bool {
        self.store().is_dir()
    }

    fn list_sessions(&self) -> Result<Vec<UnifiedSession>, String> {
        let history = self.history();
        let transcripts = transcripts(&self.store());

        let ids: BTreeSet<String> = history.keys().chain(transcripts.keys()).cloned().collect();

        let mut sessions = Vec::with_capacity(ids.len());
        for id in ids {
            let record = history.get(&id);
            let transcript = transcripts.get(&id);
            let scanned = transcript.map(|(path, _)| scan_transcript(path));
            let (scanned_title, scanned_project) = match scanned {
                Some((title, project)) => (title, project),
                None => (None, None),
            };

            let mut session = UnifiedSession::new(
                HARNESS,
                id.clone(),
                title(
                    &id,
                    record.and_then(|record| record.title.clone()),
                    scanned_title,
                ),
            );
            session.project_path = record
                .and_then(|record| record.project.clone())
                .or(scanned_project);
            session.turns_count = record.map(|record| record.prompts).unwrap_or(0);
            session.size_bytes = transcript.map(|(_, size)| *size).unwrap_or(0);
            session.created_at = record
                .and_then(|record| record.created)
                .map(utils::iso8601_from_secs);
            session.updated_at = record
                .and_then(|record| record.updated)
                .or_else(|| transcript.and_then(|(path, _)| utils::modified_secs(path)))
                .map(utils::iso8601_from_secs)
                .unwrap_or_default();

            sessions.push(session);
        }

        Ok(sessions)
    }

    fn delete_session(&self, session_id: &str) -> Result<u64, String> {
        let store = self.store();
        let mut bytes = 0;
        let mut projects = Vec::new();

        if let Ok(entries) = std::fs::read_dir(store.join("projects")) {
            for project in entries.flatten() {
                // `read_dir` follows a directory symlink and would delete the
                // transcript on the other side of the link.
                if !real_dir(&project.path()) {
                    continue;
                }
                let transcript = project.path().join(format!("{session_id}.jsonl"));
                if transcript.is_file() {
                    bytes += fsops::remove_path(&store, &transcript)?;
                    projects.push(project.path());
                }
            }
        }

        let legacy = store.join("sessions").join(format!("{session_id}.jsonl"));
        bytes += fsops::remove_path(&store, &legacy)?;

        // Leaving rows in history.jsonl is what makes the CLI re-offer a
        // conversation whose transcript is gone.
        fsops::rewrite_jsonl(&store, &store.join("history.jsonl"), &|record| {
            record.get("sessionId").and_then(|id| id.as_str()) != Some(session_id)
        })?;

        for project in projects {
            tidy_project(&store, &project);
        }

        Ok(bytes)
    }

    /// Claude keeps no database of its own: transcripts and `history.jsonl`
    /// are the whole store, so there is nothing to vacuum.
    fn optimize_storage(&self) -> Result<(), String> {
        Ok(())
    }

    /// Every conversation turn in `{session_id}.jsonl`, searched in
    /// `projects/<slug>/` and the legacy `sessions/` folder. Corrupt lines
    /// are skipped by the parser; a missing transcript is an empty feed.
    fn read_transcript(&self, session_id: &str) -> Vec<TranscriptTurn> {
        let store = self.store();

        if let Ok(projects) = std::fs::read_dir(store.join("projects")) {
            for project in projects.flatten() {
                if !real_dir(&project.path()) {
                    continue;
                }
                let candidate = project.path().join(format!("{session_id}.jsonl"));
                if candidate.is_file() {
                    return transcript::read_transcript(&candidate);
                }
            }
        }

        let legacy = store.join("sessions").join(format!("{session_id}.jsonl"));
        if legacy.is_file() {
            return transcript::read_transcript(&legacy);
        }

        Vec::new()
    }
}

/// `session_id -> (transcript, bytes)` from `projects/<slug>/` and the legacy
/// `sessions/` folder.
fn transcripts(store: &Path) -> HashMap<String, (PathBuf, u64)> {
    let mut found = HashMap::new();

    let Ok(projects) = std::fs::read_dir(store.join("projects")) else {
        return found;
    };

    for project in projects.flatten() {
        if !real_dir(&project.path()) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(project.path()) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
            {
                insert(&mut found, path);
            }
        }
    }

    if let Ok(entries) = std::fs::read_dir(store.join("sessions")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
            {
                insert(&mut found, path);
            }
        }
    }

    found
}

/// Remove a project folder that a deletion emptied, including the case where
/// only an empty `memory/` folder is left behind.
fn tidy_project(store: &Path, project: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(project) else {
        return false;
    };

    let leftover: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    let only_empty_memory = leftover.iter().all(|path| {
        path.file_name().is_some_and(|name| name == "memory")
            && std::fs::read_dir(path)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false)
    });

    if !only_empty_memory {
        return false;
    }

    for path in &leftover {
        let _ = std::fs::remove_dir(path);
    }

    fsops::remove_if_empty(store, project)
}

fn insert(found: &mut HashMap<String, (PathBuf, u64)>, path: PathBuf) {
    let Some(stem) = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
    else {
        return;
    };
    let size = utils::file_size(&path);
    found.insert(stem, (path, size));
}

/// First user prompt and recorded `cwd`, read from the transcript header only.
fn real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

fn scan_transcript(path: &Path) -> (Option<String>, Option<String>) {
    // A transcript path reached through a symlink must not be opened: `File::open`
    // follows and would take the title and cwd from outside the store.
    if utils::regular_file_len(path).is_none() {
        return (None, None);
    }
    let Ok(file) = std::fs::File::open(path) else {
        return (None, None);
    };

    let mut title = None;
    let mut project = None;

    for line in std::io::BufReader::new(file)
        .lines()
        .take(TRANSCRIPT_SCAN_LINES)
        .map_while(Result::ok)
    {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        if project.is_none() {
            project = utils::json_string(&entry, &["cwd"]);
        }

        let is_user = entry.get("type").and_then(|kind| kind.as_str()) == Some("user");
        if title.is_none() && is_user {
            title = entry
                .get("message")
                .and_then(utils::message_text)
                .map(|text| utils::trim_title(&text));
        }

        if title.is_some() && project.is_some() {
            break;
        }
    }

    (title, project)
}

fn title(id: &str, recorded: Option<String>, scanned: Option<String>) -> String {
    let text = recorded
        .filter(|text| !text.is_empty())
        .or(scanned)
        .unwrap_or_default();

    if text.is_empty() {
        format!("Session {}", id.chars().take(8).collect::<String>())
    } else {
        text
    }
}
