//! Rollout files on disk: discovery, the header that names the working
//! directory, the row deletes that follow a deletion, and the empty day/month
//! folders it leaves behind.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{MAINTENANCE_LOCK, THREAD_ID_LEN};
use crate::adapters::{fsops, utils};

/// `thread_id -> (path, bytes)` for every rollout file.
pub(super) fn rollout_files(sessions_root: &Path) -> HashMap<String, (PathBuf, u64)> {
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
pub(super) fn delete_rows_in(
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
pub(super) fn prune_empty_parents(store: &Path, file: &Path) {
    let mut current = file.parent().map(Path::to_path_buf);

    while let Some(directory) = current {
        if !fsops::remove_if_empty(store, &directory) {
            break;
        }
        current = directory.parent().map(Path::to_path_buf);
    }
}

/// The rollout header records the working directory the thread ran in.
pub(super) fn project_path(rollout: &Path) -> Option<String> {
    let header = utils::first_json_line(rollout)?;
    let payload = header.get("payload").unwrap_or(&header);
    utils::json_string(payload, &["cwd", "working_directory", "workdir"])
}

/// A real lock file means Codex is inside a maintenance pass. A symlink is
/// not that file: `exists` would follow it and refuse every delete.
pub(super) fn maintenance_lock_held(store: &Path) -> bool {
    std::fs::symlink_metadata(store.join(MAINTENANCE_LOCK))
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

pub(super) fn title(id: &str, name: Option<String>) -> String {
    match name.map(|name| utils::trim_title(&name)) {
        Some(name) if !name.is_empty() => name,
        _ => format!("Thread {}", id.chars().take(8).collect::<String>()),
    }
}
