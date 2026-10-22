//! Conversation reads for the OpenCode/Kilo lineage.
//!
//! Split from the listing module so both files stay under the line cap.
//! Three schema shapes are tried in order: the `part`+`message` join,
//! the `message` table alone, then the `part` table alone.

use std::path::Path;

use crate::types::TranscriptTurn;

use super::super::{transcript, utils};

/// First present column name, so `session_id` and `sessionID` spellings both
/// work. Names come from `PRAGMA table_info`, never from user input.
fn pick(columns: &[String], candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .find(|name| columns.contains(&name.to_string()))
        .map(|name| name.to_string())
}

/// A session id is a single row key, never a path: anything else cannot name
/// a database row and is refused before a query is built.
fn valid_id(session_id: &str) -> bool {
    !(session_id.is_empty()
        || session_id.contains('/')
        || session_id.contains('\\')
        || session_id == "."
        || session_id == "..")
}

/// Conversation turns from the `part` / `message` tables, oldest first.
///
/// Three schema shapes are tried in order — the `part`+`message` join, the
/// `message` table alone, the `part` table alone — and the first shape the
/// database actually has wins. Roles come from the message row; text comes
/// from the part payload parsed as JSON, so tool parts never surface as
/// conversation. Anything unrecognized is an empty feed, never an error.
pub fn read_transcript(root: &Path, db_rel: &str, session_id: &str) -> Vec<TranscriptTurn> {
    if !valid_id(session_id) {
        return Vec::new();
    }

    let database = root.join(db_rel);
    let Ok(connection) = utils::open_readonly(&database) else {
        return Vec::new();
    };

    if let Some(turns) = join_transcript(&connection, session_id) {
        return turns;
    }
    if let Some(turns) = message_transcript(&connection, session_id) {
        return turns;
    }
    part_transcript(&connection, session_id).unwrap_or_default()
}

/// `part` rows joined to their `message` rows: role from the message, text
/// from the part payload. Returns `None` when the schema cannot do the join.
fn join_transcript(
    connection: &rusqlite::Connection,
    session_id: &str,
) -> Option<Vec<TranscriptTurn>> {
    if !utils::table_exists(connection, "part").unwrap_or(false)
        || !utils::table_exists(connection, "message").unwrap_or(false)
    {
        return None;
    }
    let parts = utils::table_columns(connection, "part");
    let messages = utils::table_columns(connection, "message");

    let part_session = pick(&parts, &["session_id", "sessionID"]);
    let part_message = pick(&parts, &["message_id", "messageID"]);
    let message_id = pick(&messages, &["id"]);
    let role = pick(&messages, &["role"])?;
    let data = pick(&parts, &["data"])?;
    let time = pick(&parts, &["time_created", "timeCreated"])
        .or_else(|| pick(&messages, &["time_created", "timeCreated"]))
        .unwrap_or_else(|| "rowid".to_string());
    // rowid lives on `part`; a message time needs its table prefix.
    let time = if time == "rowid" {
        "p.rowid".to_string()
    } else if pick(&parts, &["time_created", "timeCreated"]).is_some() {
        format!("p.{time}")
    } else {
        format!("m.{time}")
    };

    // Both shapes need the part→message link; the session scope comes from
    // whichever side carries it. Without a link there is nothing to join on
    // and the role-less `part_transcript` below takes over.
    let (Some(link), Some(id)) = (part_message, message_id) else {
        return None;
    };
    let join = format!("JOIN message m ON m.{id} = p.{link}");
    let predicate = match part_session {
        Some(session) => format!("p.{session} = ?1"),
        None => {
            let sid = pick(&messages, &["session_id", "sessionID"])?;
            format!("p.{link} IN (SELECT {id} FROM message WHERE {sid} = ?1)")
        }
    };

    let sql = format!(
        "SELECT m.{role}, p.{data}, {time} FROM part p {join} \
         WHERE {predicate} ORDER BY {time} ASC"
    );
    let mut statement = connection.prepare(&sql).ok()?;
    let rows = statement
        .query_map([session_id], |row| {
            Ok((
                utils::text_cell(row, 0),
                utils::text_cell(row, 1),
                utils::text_cell(row, 2),
            ))
        })
        .ok()?;

    let mut turns = Vec::new();
    for row in rows.flatten() {
        let (role, data, time) = row;
        let Some(data): Option<serde_json::Value> = data
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
        else {
            continue;
        };
        let normalized = serde_json::json!({
            "role": role,
            "message": data,
            "timestamp": time,
        });
        if let Some(turn) = transcript::parse_value(&normalized) {
            turns.push(turn);
            if turns.len() >= transcript::MAX_TRANSCRIPT_TURNS {
                break;
            }
        }
    }
    Some(turns)
}

/// The `message` table alone: role plus a plain-text content column.
/// Returns `None` when the table or columns are absent.
fn message_transcript(
    connection: &rusqlite::Connection,
    session_id: &str,
) -> Option<Vec<TranscriptTurn>> {
    if !utils::table_exists(connection, "message").unwrap_or(false) {
        return None;
    }
    let columns = utils::table_columns(connection, "message");
    let session = pick(&columns, &["session_id", "sessionID"])?;
    let role = pick(&columns, &["role"])?;
    let content = pick(&columns, &["content", "text", "body", "data"])?;
    let time = pick(
        &columns,
        &["time_created", "timeCreated", "timestamp", "ts"],
    );

    let select_time = time.clone().unwrap_or_else(|| "NULL".to_string());
    let order = time.unwrap_or_else(|| "rowid".to_string());
    let sql = format!(
        "SELECT {role}, {content}, {select_time} FROM message \
         WHERE {session} = ?1 ORDER BY {order} ASC"
    );
    let mut statement = connection.prepare(&sql).ok()?;
    let rows = statement
        .query_map([session_id], |row| {
            Ok((
                utils::text_cell(row, 0),
                utils::text_cell(row, 1),
                utils::text_cell(row, 2),
            ))
        })
        .ok()?;

    let mut turns = Vec::new();
    for row in rows.flatten() {
        let (role, content, time) = row;
        let (Some(role), Some(content)) = (role, content) else {
            continue;
        };
        if content.trim().is_empty() {
            continue;
        }
        // Plain text rows build directly; JSON payloads go through the parser
        // so part-shaped content is filtered the same way everywhere.
        let turn = match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(data) if data.is_object() || data.is_array() => {
                let normalized = serde_json::json!({
                    "role": role,
                    "message": data,
                    "timestamp": time,
                });
                transcript::parse_value(&normalized)
            }
            _ => {
                let stamped = time
                    .filter(|stamp| !stamp.trim().is_empty())
                    .and_then(|stamp| utils::timestamp_secs(&stamp))
                    .map(utils::iso8601_from_secs);
                transcript::build_turn(&role, &content, stamped)
            }
        };
        if let Some(turn) = turn {
            turns.push(turn);
            if turns.len() >= transcript::MAX_TRANSCRIPT_TURNS {
                break;
            }
        }
    }
    Some(turns)
}

/// The `part` table alone: roles come from each payload's own `role` field.
/// Returns `None` when the table cannot serve the session.
fn part_transcript(
    connection: &rusqlite::Connection,
    session_id: &str,
) -> Option<Vec<TranscriptTurn>> {
    if !utils::table_exists(connection, "part").unwrap_or(false) {
        return None;
    }
    let columns = utils::table_columns(connection, "part");
    let session = pick(&columns, &["session_id", "sessionID"])?;
    let data = pick(&columns, &["data"])?;
    let time = pick(&columns, &["time_created", "timeCreated"]);

    let select_time = time.clone().unwrap_or_else(|| "NULL".to_string());
    let order = time.unwrap_or_else(|| "rowid".to_string());
    let sql = format!(
        "SELECT {data}, {select_time} FROM part \
         WHERE {session} = ?1 ORDER BY {order} ASC"
    );
    let mut statement = connection.prepare(&sql).ok()?;
    let rows = statement
        .query_map([session_id], |row| {
            Ok((utils::text_cell(row, 0), utils::text_cell(row, 1)))
        })
        .ok()?;

    let mut turns = Vec::new();
    for row in rows.flatten() {
        let (data, time) = row;
        let Some(data): Option<serde_json::Value> = data
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
        else {
            continue;
        };
        let normalized = serde_json::json!({
            "role": data.get("role"),
            "message": data,
            "timestamp": time,
        });
        if let Some(turn) = transcript::parse_value(&normalized) {
            turns.push(turn);
            if turns.len() >= transcript::MAX_TRANSCRIPT_TURNS {
                break;
            }
        }
    }
    Some(turns)
}
