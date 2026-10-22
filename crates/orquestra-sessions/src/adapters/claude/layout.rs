//! Store layout: where a session's transcript lives, what its header says,
//! and how an emptied project folder is tidied away.
//!
//! Every path here goes through [`real_dir`] or [`utils::file_size`], so a
//! directory or file symlink is never followed out of `~/.claude`.

use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use super::super::{fsops, utils};
use super::TRANSCRIPT_SCAN_LINES;

/// `session_id -> (transcript, bytes)` from `projects/<slug>/` and the legacy
/// `sessions/` folder.
pub(super) fn transcripts(store: &Path) -> HashMap<String, (PathBuf, u64)> {
    let mut found = HashMap::new();

    let projects_dir = store.join("projects");
    if !real_dir(&projects_dir) {
        // Missing `projects/` still skips the legacy walk, as before. A
        // symlink is present and must not be opened, but sessions/ still is.
        if std::fs::symlink_metadata(&projects_dir).is_err() {
            return found;
        }
    } else if let Ok(projects) = std::fs::read_dir(&projects_dir) {
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
    }

    // Same hole as a project symlink: `read_dir` follows `sessions/` and the
    // last component is still a regular file, so the scan would open it.
    let sessions = store.join("sessions");
    if real_dir(&sessions) {
        if let Ok(entries) = std::fs::read_dir(&sessions) {
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
    }

    found
}

/// Remove a project folder that a deletion emptied, including the case where
/// only an empty `memory/` folder is left behind.
pub(super) fn tidy_project(store: &Path, project: &Path) -> bool {
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

/// An id that stays one path segment. `{id}.jsonl` is joined under every
/// project, so `../secret` would name a regular file outside the store.
pub(super) fn single_segment(id: &str) -> bool {
    !id.is_empty() && !id.contains('/') && !id.contains('\\') && id != "." && id != ".."
}

/// `symlink_metadata` plus an explicit type check: `is_dir()` follows links.
pub(super) fn real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

/// First user prompt and recorded `cwd`, read from the transcript header only.
pub(super) fn scan_transcript(path: &Path) -> (Option<String>, Option<String>) {
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

pub(super) fn title(id: &str, recorded: Option<String>, scanned: Option<String>) -> String {
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
