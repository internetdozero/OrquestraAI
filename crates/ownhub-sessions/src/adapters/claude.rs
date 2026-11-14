//! Claude Code: one transcript per session under `projects/<slug>/`, plus a
//! `history.jsonl` of prompts used for titles and activity.
//!
//! Project folder names are encoded paths (`-home-system`), which are lossy
//! when a directory contains a dash. The transcript itself records `cwd`, so
//! that is preferred and the slug is only a last resort.
//!
//! [`history`] turns the prompt index into titles and activity, and
//! [`layout`] owns where a transcript lives and what its header says.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::types::{TranscriptTurn, UnifiedSession};

use super::{fsops, transcript, utils, SessionAdapter};
use layout::{real_dir, scan_transcript, single_segment, tidy_project, title, transcripts};

mod history;
mod layout;

const HARNESS: &str = "claude";
const STORE_REL: &str = ".claude";
/// Enough lines to reach the first user prompt: transcripts open with a mode,
/// permission mode and atis latch before any message.
const TRANSCRIPT_SCAN_LINES: usize = 40;

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
        // `{id}.jsonl` is joined under each project and under legacy sessions/.
        // A directory symlink named `escape` plus id `escape/secret` deletes
        // the outside file. `read_transcript` already refuses that id.
        if !single_segment(session_id) {
            return Ok(0);
        }
        let store = self.store();
        let mut bytes = 0;
        let mut projects = Vec::new();

        // A symlink at `projects/` is followed by `read_dir`. Each child is
        // then a real directory, so the per-project check does not stop it.
        let projects_dir = store.join("projects");
        if real_dir(&projects_dir) {
            if let Ok(entries) = std::fs::read_dir(&projects_dir) {
                for project in entries.flatten() {
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
        }

        // `sessions` itself may be a directory symlink. The joined jsonl is
        // then a regular file, so `remove_path` would delete the outside target.
        let sessions_dir = store.join("sessions");
        if real_dir(&sessions_dir) {
            let legacy = sessions_dir.join(format!("{session_id}.jsonl"));
            bytes += fsops::remove_path(&store, &legacy)?;
        }

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
        // The id is joined onto `{id}.jsonl`. `../secret` still names a
        // regular file after the kernel resolves it.
        if !single_segment(session_id) {
            return Vec::new();
        }
        let store = self.store();

        let projects_dir = store.join("projects");
        if real_dir(&projects_dir) {
            if let Ok(projects) = std::fs::read_dir(&projects_dir) {
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
        }

        let sessions_dir = store.join("sessions");
        if real_dir(&sessions_dir) {
            let legacy = sessions_dir.join(format!("{session_id}.jsonl"));
            if legacy.is_file() {
                return transcript::read_transcript(&legacy);
            }
        }

        Vec::new()
    }
}
