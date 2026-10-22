//! Blackboard rows: post, list, mark read.
//!
//! Same semantics as the Python MCP — topics lowercased, target defaults to
//! `all`, oldest notes first.

use rusqlite::{params_from_iter, Connection};

use super::store::{db_error, get_note, note_from_row, require_text};
use super::types::{BlackboardNote, PostNoteInput};

/// Post a note; the topic is lowercased and the target defaults to `all`.
pub fn post_note(connection: &Connection, input: PostNoteInput) -> Result<BlackboardNote, String> {
    let project = require_text(&input.project, "project")?;
    let topic = require_text(&input.topic, "topic")?.to_lowercase();
    let sender = require_text(&input.sender, "sender")?;
    let target = match input.target.as_deref() {
        None => "all".to_string(),
        Some(raw) => require_text(raw, "target")?,
    };
    let note = require_text(&input.note, "note")?;

    connection
        .execute(
            "INSERT INTO blackboard_notes (project, topic, sender, target, note) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![project, topic, sender, target, note],
        )
        .map_err(db_error)?;

    get_note(connection, connection.last_insert_rowid())
}

/// Read notes oldest first, optionally unread only and/or one project.
pub fn list_notes(
    connection: &Connection,
    project: Option<&str>,
    unread_only: bool,
) -> Result<Vec<BlackboardNote>, String> {
    let mut clauses = Vec::new();
    let mut params: Vec<String> = Vec::new();

    if let Some(raw) = project {
        if !raw.trim().is_empty() {
            clauses.push("project = ?");
            params.push(require_text(raw, "project")?);
        }
    }
    if unread_only {
        clauses.push("is_read = 0");
    }

    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {} ", clauses.join(" AND "))
    };
    let sql = format!("SELECT * FROM blackboard_notes {where_sql}ORDER BY created_at ASC, id ASC");

    let mut statement = connection.prepare(&sql).map_err(db_error)?;
    let notes = statement
        .query_map(params_from_iter(params.iter()), note_from_row)
        .map_err(db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error)?;
    Ok(notes)
}

/// Mark one note read; a missing id is an error.
pub fn mark_note_read(connection: &Connection, id: i64) -> Result<(), String> {
    let changed = connection
        .execute(
            "UPDATE blackboard_notes SET is_read = 1 WHERE id = ?1",
            [id],
        )
        .map_err(db_error)?;
    if changed == 0 {
        return Err(format!("note #{id} not found"));
    }
    Ok(())
}
