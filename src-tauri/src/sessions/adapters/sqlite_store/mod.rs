//! Shared reader for the OpenCode/Kilo lineage.
//!
//! Both ship the same schema — a `session` table plus per-session artifact
//! folders — so one tolerant query serves both adapters and a third member of
//! that lineage costs a single line in its adapter.

use std::path::{Path, PathBuf};

use crate::sessions::types::UnifiedSession;

use super::fsops;
use super::utils::{self, text_cell, timestamp_secs, trim_title};

mod transcript;

pub use transcript::read_transcript;

/// List sessions from `<root>/<db_rel>`.
///
/// Columns are discovered instead of assumed: the lineage has added columns
/// across releases, and a store missing `title` should still list its sessions
/// rather than fail the harness.
pub fn list(
    root: &Path,
    harness: &str,
    db_rel: &str,
    storage_rel: &str,
) -> Result<Vec<UnifiedSession>, String> {
    let database = root.join(db_rel);
    let connection = utils::open_readonly(&database)?;

    if !utils::table_exists(&connection, "session")? {
        return Ok(Vec::new());
    }

    let columns = utils::table_columns(&connection, "session");
    if !columns.contains(&"id".to_string()) {
        return Err(format!("{} has no `session.id` column", database.display()));
    }

    let pick = |name: &str| {
        if columns.contains(&name.to_string()) {
            name.to_string()
        } else {
            "NULL".to_string()
        }
    };
    let sql = format!(
        "SELECT id, {}, {}, {}, {} FROM session",
        pick("title"),
        pick("directory"),
        pick("time_updated"),
        pick("time_created")
    );

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("{}: {error}", database.display()))?;

    let rows = statement
        .query_map([], |row| {
            Ok((
                text_cell(row, 0),
                text_cell(row, 1),
                text_cell(row, 2),
                text_cell(row, 3),
                text_cell(row, 4),
            ))
        })
        .map_err(|error| format!("{}: {error}", database.display()))?;

    let mut sessions = Vec::new();
    for row in rows.flatten() {
        let (Some(id), title, directory, updated, created) = row else {
            continue;
        };
        if id.is_empty() {
            continue;
        }

        let mut session = UnifiedSession::new(harness, id.clone(), fallback_title(&id, title));
        session.project_path = directory.filter(|path| !path.trim().is_empty());
        session.updated_at = updated
            .as_deref()
            .and_then(timestamp_secs)
            .map(utils::iso8601_from_secs)
            .or_else(|| utils::modified_secs(&database).map(utils::iso8601_from_secs))
            .unwrap_or_default();
        session.created_at = created
            .as_deref()
            .and_then(timestamp_secs)
            .map(utils::iso8601_from_secs);
        session.size_bytes = artifact_size(root, storage_rel, &id);

        sessions.push(session);
    }

    Ok(sessions)
}

/// Delete one session: its artifact folders, then the row that cascades to
/// `message`, `part`, `todo` and the rest of the schema.
///
/// The cascade only fires with foreign keys enabled on this connection, which
/// is why the pragma is set before the delete rather than inherited.
pub fn delete(root: &Path, db_rel: &str, storage_rel: &str, id: &str) -> Result<u64, String> {
    let mut bytes = 0;
    for folder in artifact_dirs(root, storage_rel, id) {
        bytes += fsops::remove_path(root, &folder)?;
    }

    let database = root.join(db_rel);
    if database.is_file() {
        let connection = fsops::open_writable(&database)?;
        fsops::enable_cascade(&connection)?;
        fsops::delete_rows(&connection, "session", "id", id)?;
    }

    Ok(bytes)
}

/// Reclaim the pages the cascaded deletes left in the session database.
pub fn vacuum(root: &Path, db_rel: &str) -> Result<(), String> {
    fsops::vacuum(&root.join(db_rel))
}

/// Artifact folders this session owns, newest layout first.
/// Later releases moved them under `storage/`; older ones kept them at the
/// store root, so both are returned and both are measured.
fn artifact_dirs(root: &Path, storage_rel: &str, id: &str) -> Vec<PathBuf> {
    let storage = root.join(storage_rel);

    ["session_diff", "tool-output", "snapshot"]
        .iter()
        .flat_map(|folder| [storage.join(folder), storage.join("storage").join(folder)])
        .map(|folder| folder.join(id))
        .filter(|path| artifact_dir_is_inside(&storage, path))
        .collect()
}

/// `path.is_dir()` follows a symlink ancestor, so `session_diff/<id>` can be a
/// real directory that lives outside the store. Every directory between the
/// store and the session folder has to be real. A symlink session folder is
/// still removed: `remove_path` unlinks it and does not follow.
fn artifact_dir_is_inside(store: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(store) else {
        return false;
    };
    let mut current = store.to_path_buf();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        if components.peek().is_none() {
            break;
        }
        current.push(component);
        if !real_dir(&current) {
            return false;
        }
    }
    path.is_dir()
}

fn real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

/// Bytes of the per-session artifact folders, when this release keeps any.
fn artifact_size(root: &Path, storage_rel: &str, id: &str) -> u64 {
    artifact_dirs(root, storage_rel, id)
        .iter()
        .map(|path| utils::dir_size(path))
        .sum()
}

fn fallback_title(id: &str, title: Option<String>) -> String {
    match title.map(|title| trim_title(&title)) {
        Some(title) if !title.is_empty() => title,
        _ => format!("Session {}", id.chars().take(8).collect::<String>()),
    }
}
