//! SQLite plumbing for the native Idea Vault: path resolution, connection
//! setup, the canonical schema and the validation rules shared with the
//! Python MCP. Query logic lives in `ideas_repo` and `notes_repo` so this
//! file stays a thin foundation.

/// Canonical schema, kept identical to the Python MCP's `SCHEMA`.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS ideas (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project     TEXT NOT NULL,
    title       TEXT NOT NULL,
    description TEXT,
    category    TEXT NOT NULL DEFAULT 'feature',
    priority    TEXT NOT NULL DEFAULT 'medium',
    tags        TEXT,
    status      TEXT NOT NULL DEFAULT 'open',
    created_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP,
    prompt      TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS blackboard_notes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project     TEXT NOT NULL,
    topic       TEXT NOT NULL,
    sender      TEXT NOT NULL,
    target      TEXT NOT NULL DEFAULT 'all',
    note        TEXT NOT NULL,
    is_read     BOOLEAN NOT NULL DEFAULT 0,
    created_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_ideas_project_status ON ideas (project, status);
CREATE INDEX IF NOT EXISTS idx_ideas_category ON ideas (category);
CREATE INDEX IF NOT EXISTS idx_notes_project_topic ON blackboard_notes (project, topic, is_read);
";

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, Row};

use super::types::{BlackboardNote, IdeaItem};

/// Database path, honoring `IDEA_VAULT_DB_PATH` and `XDG_DATA_HOME`.
pub fn resolve_db_path() -> Result<PathBuf, String> {
    let trimmed = std::env::var("IDEA_VAULT_DB_PATH")
        .map(|raw| raw.trim().to_string())
        .unwrap_or_default();
    if !trimmed.is_empty() {
        return Ok(PathBuf::from(trimmed));
    }

    let data_home = std::env::var("XDG_DATA_HOME")
        .map(|raw| raw.trim().to_string())
        .unwrap_or_default();
    if !data_home.is_empty() {
        return Ok(PathBuf::from(data_home).join("idea-vault/ideas.sqlite3"));
    }

    Ok(crate::paths::require_home("locate the idea vault")?
        .join(".local/share/idea-vault/ideas.sqlite3"))
}

/// Open the vault: create parents, wait on writers, enable WAL, ensure schema.
pub fn connect_at(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
    }

    let connection = Connection::open(path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("cannot set a busy timeout: {error}"))?;
    connection
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
        .map_err(|error| format!("cannot enable WAL: {error}"))?;
    connection
        .execute_batch(SCHEMA)
        .map_err(|error| format!("cannot initialize the schema: {error}"))?;
    migrate_completed_at(&connection)?;
    migrate_prompt(&connection)?;

    Ok(connection)
}

/// Idempotent migration for vaults created before `completed_at` existed:
/// `CREATE TABLE IF NOT EXISTS` never adds the column to an old table, so
/// patch it here instead of failing reads written before the migration.
fn migrate_completed_at(connection: &Connection) -> Result<(), String> {
    let has_column: bool = connection
        .prepare("SELECT 1 FROM pragma_table_info('ideas') WHERE name = 'completed_at'")
        .and_then(|mut statement| statement.exists([]))
        .map_err(|error| format!("cannot inspect the ideas schema: {error}"))?;
    if !has_column {
        connection
            .execute_batch("ALTER TABLE ideas ADD COLUMN completed_at TIMESTAMP;")
            .map_err(|error| format!("cannot migrate the ideas schema: {error}"))?;
    }
    // Backfill legacy completed/archived ideas so historical velocity & heatmap ignite
    connection
        .execute_batch(
            "UPDATE ideas SET completed_at = updated_at \
             WHERE status IN ('completed', 'archived') AND completed_at IS NULL;",
        )
        .map_err(|error| format!("cannot backfill completed_at: {error}"))?;
    Ok(())
}

/// Idempotent migration for vaults created before `prompt` existed:
/// old tables gain the Orchestrator Builder-prompt column with an empty
/// default so legacy rows stay readable.
fn migrate_prompt(connection: &Connection) -> Result<(), String> {
    let has_column: bool = connection
        .prepare("SELECT 1 FROM pragma_table_info('ideas') WHERE name = 'prompt'")
        .and_then(|mut statement| statement.exists([]))
        .map_err(|error| format!("cannot inspect the ideas schema: {error}"))?;
    if !has_column {
        connection
            .execute_batch("ALTER TABLE ideas ADD COLUMN prompt TEXT NOT NULL DEFAULT '';")
            .map_err(|error| format!("cannot migrate the ideas schema: {error}"))?;
    }
    Ok(())
}

/// Open the vault at its resolved path.
pub fn connect() -> Result<Connection, String> {
    connect_at(&resolve_db_path()?)
}

// ---------------------------------------------------------------------------
// Validation (same rules as the MCP)
// ---------------------------------------------------------------------------

/// Default page size when the caller passes no limit: the vault is personal
/// scale, so this is effectively complete, while the IPC payload stays
/// bounded no matter how long the app runs.
pub(super) const PAGE_DEFAULT_LIMIT: i64 = 500;
/// Hard ceiling: even an explicit huge limit cannot turn a list call into
/// an unbounded `SELECT *`.
pub(super) const PAGE_MAX_LIMIT: i64 = 2000;

/// Clamp caller paging into `(limit, offset)`. Both interpolate into SQL as
/// plain integers (never as bound text), so clamping here is also the
/// injection story: no caller input reaches the statement as SQL.
pub(super) fn clamp_page(limit: Option<i64>, offset: Option<i64>) -> (i64, i64) {
    let capped = limit.unwrap_or(PAGE_DEFAULT_LIMIT).clamp(1, PAGE_MAX_LIMIT);
    let skipped = offset.unwrap_or(0).max(0);
    (capped, skipped)
}

pub(super) fn require_text(value: &str, field: &str) -> Result<String, String> {
    let text = value.trim();
    if text.is_empty() {
        return Err(format!("{field} must be a non-empty string"));
    }
    Ok(text.to_string())
}

pub(super) fn require_choice(value: &str, allowed: &[&str], field: &str) -> Result<String, String> {
    let text = value.trim().to_lowercase();
    if allowed.contains(&text.as_str()) {
        Ok(text)
    } else {
        Err(format!("{field} must be one of: {}", allowed.join(", ")))
    }
}

/// `None`, `""` and `"all"` mean no filter; anything else must be valid.
pub(super) fn optional_filter(
    value: Option<&str>,
    allowed: &[&str],
    field: &str,
) -> Result<Option<String>, String> {
    match value {
        None => Ok(None),
        Some(raw) => {
            let text = raw.trim().to_lowercase();
            if text.is_empty() || text == "all" {
                Ok(None)
            } else if allowed.contains(&text.as_str()) {
                Ok(Some(text))
            } else {
                Err(format!("{field} must be one of: {}", allowed.join(", ")))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Row mapping
// ---------------------------------------------------------------------------

pub(super) fn idea_from_row(row: &Row) -> Result<IdeaItem, rusqlite::Error> {
    Ok(IdeaItem {
        id: row.get("id")?,
        project: row.get("project")?,
        title: row.get("title")?,
        description: row.get("description")?,
        category: row.get("category")?,
        priority: row.get("priority")?,
        tags: row.get("tags")?,
        status: row.get("status")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        completed_at: row.get("completed_at")?,
        prompt: row
            .get::<_, Option<String>>("prompt")?
            .filter(|text| !text.trim().is_empty()),
    })
}

pub(super) fn note_from_row(row: &Row) -> Result<BlackboardNote, rusqlite::Error> {
    Ok(BlackboardNote {
        id: row.get("id")?,
        project: row.get("project")?,
        topic: row.get("topic")?,
        sender: row.get("sender")?,
        target: row.get("target")?,
        note: row.get("note")?,
        is_read: row.get::<_, i64>("is_read")? != 0,
        created_at: row.get("created_at")?,
    })
}

pub(super) fn db_error(error: rusqlite::Error) -> String {
    format!("idea vault query failed: {error}")
}

pub(super) fn get_idea(connection: &Connection, id: i64) -> Result<IdeaItem, String> {
    connection
        .query_row("SELECT * FROM ideas WHERE id = ?1", [id], idea_from_row)
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => format!("idea #{id} not found"),
            other => db_error(other),
        })
}

pub(super) fn get_note(connection: &Connection, id: i64) -> Result<BlackboardNote, String> {
    connection
        .query_row(
            "SELECT * FROM blackboard_notes WHERE id = ?1",
            [id],
            note_from_row,
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => format!("note #{id} not found"),
            other => db_error(other),
        })
}
