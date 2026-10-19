//! Freebuff Desktop's native store: one `desktop-v2.db` per project.
//!
//! Each project under `.config/freebuff-desktop/projects/<slug>/` keeps a
//! `project.json` (with the real `projectPath`) and a `desktop-v2.db` whose
//! `threads` table holds the chat history and whose `messages` table holds one
//! row per turn (`seq` primary key, `parts_json` payload). A
//! `freebuff_conversation_search` FTS table mirrors message text, maintained
//! by triggers on `messages`.
//!
//! Listing is read-only (`query_only`); deletion removes the thread's rows,
//! its FTS entries, its per-thread folder under `local-projects/*/threads/`
//! or `no-folder/threads/`, and a same-id worktree when one exists. Session
//! ids are `desktop:<slug>:<thread-id>` so they never collide with the bare
//! worktree ids or the `project/chat` Manicode ids.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::sessions::types::UnifiedSession;

use super::{fsops, utils};

mod transcript;
mod worktree;

pub use transcript::transcript;
pub use worktree::{claimed_dir_names, find_worktree};

const HARNESS: &str = "freebuff";
/// Prefix of every desktop-thread session id: `desktop:<slug>:<thread-id>`.
pub const PREFIX: &str = "desktop:";
const PROJECTS_REL: &str = ".config/freebuff-desktop/projects";
const LOCAL_REL: &str = ".config/freebuff-desktop/local-projects";
const NOFOLDER_REL: &str = ".config/freebuff-desktop/no-folder/threads";
const WORKTREES_REL: &str = ".freebuff/worktrees";
const DB_NAME: &str = "desktop-v2.db";
const FTS_TABLE: &str = "freebuff_conversation_search";

/// The desktop store exists when at least one project directory does.
pub fn is_available(home: &Path) -> bool {
    home.join(PROJECTS_REL).is_dir()
}

/// Split `desktop:<slug>:<thread-id>` into its parts. Anything else is not a
/// desktop thread. Dot segments are refused outright: a slug is always a
/// single directory name from the projects listing, never a path.
pub fn parse_id(session_id: &str) -> Option<(String, String)> {
    let rest = session_id.strip_prefix(PREFIX)?;
    let (slug, thread) = rest.split_once(':')?;
    if slug.is_empty() || thread.is_empty() {
        return None;
    }
    if slug.contains('/') || thread.contains('/') || thread.contains(':') {
        return None;
    }
    if slug == "." || slug == ".." || thread == "." || thread == ".." {
        return None;
    }
    Some((slug.to_string(), thread.to_string()))
}

/// Every desktop thread in every project. A project whose database is locked
/// or corrupt is skipped: one broken store must not hide the healthy ones.
pub fn list(home: &Path) -> Vec<UnifiedSession> {
    let projects = home.join(PROJECTS_REL);
    let Ok(entries) = std::fs::read_dir(&projects) else {
        return Vec::new();
    };

    let mut sessions = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().into_owned();
        let database = entry.path().join(DB_NAME);
        if !database.is_file() {
            continue;
        }

        let project_path = desktop_project_path(&entry.path());
        let Ok(connection) = utils::open_readonly(&database) else {
            continue;
        };
        let Ok(rows) = query_threads(&connection, &database) else {
            continue;
        };

        for row in rows {
            if row.id.contains('/') || row.id.contains(':') {
                continue;
            }
            let mut session = UnifiedSession::new(
                HARNESS,
                format!("{PREFIX}{slug}:{}", row.id),
                fallback_title(&row.id, row.title),
            );
            session.project_path = project_path
                .clone()
                .or_else(|| row.project_path.filter(|path| !path.trim().is_empty()));
            session.updated_at = row
                .updated_ms
                .map(|ms| utils::iso8601_from_secs(ms.div_euclid(1_000)))
                .or_else(|| {
                    utils::modified_secs(&database).map(utils::iso8601_from_secs)
                })
                .unwrap_or_default();
            session.created_at = row
                .created_ms
                .map(|ms| utils::iso8601_from_secs(ms.div_euclid(1_000)));
            session.turns_count = row.turns;
            session.size_bytes =
                row.message_bytes + extra_size(home, &row.id);

            sessions.push(session);
        }
    }

    sessions
}

/// Delete one desktop thread: its message rows, its thread row, its FTS
/// entries, its per-thread folder and a same-id worktree. Returns the bytes
/// of folders removed; database pages are reclaimed by [`vacuum_all`].
pub fn delete(home: &Path, slug: &str, thread: &str) -> Result<u64, String> {
    if slug.contains('/') || thread.contains('/') || thread.contains(':') {
        return Err(format!("refusing to delete desktop thread {thread}: bad id"));
    }

    let database = home.join(PROJECTS_REL).join(slug).join(DB_NAME);
    if database.is_file() {
        delete_rows(&database, thread)?;
    }

    let mut bytes = 0;
    for folder in thread_dirs(home, thread) {
        bytes += fsops::remove_path(home, &folder)?;
    }

    let worktrees = home.join(WORKTREES_REL);
    if let Some(matched) = find_worktree(&worktrees, thread) {
        bytes += fsops::remove_path(&worktrees, &matched)?;
    }

    Ok(bytes)
}

/// Reclaim the pages deletes left in every desktop database.
pub fn vacuum_all(home: &Path) -> Result<(), String> {
    let projects = home.join(PROJECTS_REL);
    let Ok(entries) = std::fs::read_dir(&projects) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let database = entry.path().join(DB_NAME);
        if database.is_file() {
            fsops::vacuum(&database)?;
        }
    }

    Ok(())
}

/// The `projectPath` from `project.json`, when the file exists and names one.
fn desktop_project_path(project: &Path) -> Option<String> {
    let meta = utils::read_json(&project.join("project.json"))?;
    let path = meta.get("projectPath")?.as_str()?.trim();
    (!path.is_empty()).then(|| path.to_string())
}

struct ThreadRow {
    id: String,
    title: Option<String>,
    project_path: Option<String>,
    created_ms: Option<i64>,
    updated_ms: Option<i64>,
    turns: usize,
    message_bytes: u64,
}

/// One grouped query per database: the thread plus its message count and the
/// bytes of its message payloads. Columns are discovered, not assumed, so a
/// release that renames one still lists its threads.
fn query_threads(connection: &Connection, database: &Path) -> Result<Vec<ThreadRow>, String> {
    if !utils::table_exists(connection, "threads")? {
        return Ok(Vec::new());
    }
    let columns = utils::table_columns(connection, "threads");
    if !columns.contains(&"id".to_string()) {
        return Err(format!("{} has no `threads.id` column", database.display()));
    }

    let pick = |name: &str| {
        if columns.contains(&name.to_string()) {
            format!("t.{name}")
        } else {
            "NULL".to_string()
        }
    };

    let message_columns = if utils::table_exists(connection, "messages")? {
        utils::table_columns(connection, "messages")
    } else {
        Vec::new()
    };
    let joined = message_columns.contains(&"thread_id".to_string());
    let count_expr = if joined && message_columns.contains(&"seq".to_string()) {
        "COUNT(m.seq)".to_string()
    } else if joined {
        "COUNT(m.thread_id)".to_string()
    } else {
        "0".to_string()
    };
    let mut size_terms = Vec::new();
    for column in ["parts_json", "attachments_json"] {
        if joined && message_columns.contains(&column.to_string()) {
            size_terms.push(format!("COALESCE(SUM(LENGTH(m.{column})), 0)"));
        }
    }
    let size_expr = if size_terms.is_empty() {
        "0".to_string()
    } else {
        size_terms.join(" + ")
    };

    // The join is conditional: a database without a `messages` table still
    // lists its threads with zero turns rather than failing the project.
    let join = if joined {
        "LEFT JOIN messages m ON m.thread_id = t.id"
    } else {
        ""
    };
    // Positional `ORDER BY 5`: the alias is `NULL` when this release has no
    // `updated_at`, and ordering by a missing identifier would fail.
    let sql = format!(
        "SELECT t.id, {}, {}, {}, {}, {count_expr}, {size_expr} \
         FROM threads t {join} \
         GROUP BY t.id ORDER BY 5 DESC",
        pick("title"),
        pick("project_path"),
        pick("created_at"),
        pick("updated_at"),
    );

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("{}: {error}", database.display()))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                utils::text_cell(row, 0),
                utils::text_cell(row, 1),
                utils::text_cell(row, 2),
                utils::i64_cell(row, 3),
                utils::i64_cell(row, 4),
                utils::i64_cell(row, 5),
                utils::i64_cell(row, 6),
            ))
        })
        .map_err(|error| format!("{}: {error}", database.display()))?;

    let mut threads = Vec::new();
    for row in rows.flatten() {
        let (Some(id), title, project_path, created_ms, updated_ms, turns, message_bytes) = row
        else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        threads.push(ThreadRow {
            id,
            title,
            project_path,
            created_ms,
            updated_ms,
            turns: turns.unwrap_or(0).max(0) as usize,
            message_bytes: message_bytes.unwrap_or(0).max(0) as u64,
        });
    }

    Ok(threads)
}

/// Remove the thread's rows, then its FTS entries. The FTS triggers already
/// mirror message deletes, so the explicit cleanup is a belt-and-braces for
/// databases whose triggers differ.
fn delete_rows(database: &Path, thread: &str) -> Result<(), String> {
    let connection = fsops::open_writable(database)?;
    fsops::delete_rows(&connection, "messages", "thread_id", thread)?;
    fsops::delete_rows(&connection, "threads", "id", thread)?;

    if utils::table_exists(&connection, FTS_TABLE)?
        && utils::table_columns(&connection, FTS_TABLE)
            .iter()
            .any(|name| name == "thread_id")
    {
        connection
            .execute(
                &format!("DELETE FROM {FTS_TABLE} WHERE thread_id = ?1"),
                [thread],
            )
            .map_err(|error| format!("DELETE FROM {FTS_TABLE} failed: {error}"))?;
    }

    Ok(())
}

/// Per-thread folders that belong to this thread id, newest layout first:
/// every `local-projects/<pid>/threads/<id>` plus `no-folder/threads/<id>`.
fn thread_dirs(home: &Path, thread: &str) -> Vec<PathBuf> {
    let mut folders = Vec::new();

    if let Ok(entries) = std::fs::read_dir(home.join(LOCAL_REL)) {
        for entry in entries.flatten() {
            let folder = entry.path().join("threads").join(thread);
            if folder.is_dir() {
                folders.push(folder);
            }
        }
    }

    let orphan = home.join(NOFOLDER_REL).join(thread);
    if orphan.is_dir() {
        folders.push(orphan);
    }

    folders
}

/// Message payload bytes already counted in SQL, plus the thread folders and
/// the correlated worktree (exact UUID or 8-char prefix) on disk.
fn extra_size(home: &Path, thread: &str) -> u64 {
    let mut bytes: u64 = thread_dirs(home, thread)
        .iter()
        .map(|folder| utils::dir_size(folder))
        .sum();
    let worktrees = home.join(WORKTREES_REL);
    if let Some(matched) = find_worktree(&worktrees, thread) {
        bytes += utils::dir_size(&matched);
    }
    bytes
}

fn fallback_title(id: &str, title: Option<String>) -> String {
    match title.map(|title| utils::trim_title(&title)) {
        Some(title) if !title.is_empty() => title,
        _ => format!("Thread {}", id.chars().take(8).collect::<String>()),
    }
}
