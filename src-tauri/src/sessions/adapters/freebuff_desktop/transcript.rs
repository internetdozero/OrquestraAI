//! Transcript reads for Freebuff Desktop threads.
//!
//! Split from the listing module so both files stay under the line cap.

use std::path::Path;

use crate::sessions::types::TranscriptTurn;

use super::super::{time, transcript, utils};
use super::{real_dir, DB_NAME, PROJECTS_REL};

/// Conversation turns of one desktop thread, oldest first.
///
/// Columns are discovered, not assumed: the text comes from `parts_json`
/// (falling back to `content` / `text` / `body`), the clock from
/// `created_at` (falling back to `timestamp`), ordered by `seq` when the
/// release has one. A database without a `messages` table, or rows whose
/// role is not user/assistant, report an empty feed — never an error.
pub fn transcript(home: &Path, slug: &str, thread: &str) -> Vec<TranscriptTurn> {
    const LIMIT: usize = 1000;

    if slug.contains('/') || thread.contains('/') || thread.contains(':') {
        return Vec::new();
    }
    let projects = home.join(PROJECTS_REL);
    if !real_dir(&projects) {
        return Vec::new();
    }
    let database = projects.join(slug).join(DB_NAME);
    let Ok(connection) = utils::open_readonly(&database) else {
        return Vec::new();
    };
    let Ok(true) = utils::table_exists(&connection, "messages") else {
        return Vec::new();
    };
    let columns = utils::table_columns(&connection, "messages");
    if !columns.iter().any(|name| name == "thread_id") {
        return Vec::new();
    }
    let role_col = pick_column(&columns, &["role"]);
    let text_col = pick_column(&columns, &["parts_json", "content", "text", "body"]);
    let (Some(role_col), Some(text_col)) = (role_col, text_col) else {
        return Vec::new();
    };
    let ts_col = pick_column(&columns, &["created_at", "timestamp", "updated_at", "ts"]);
    let order_col = pick_column(&columns, &["seq", "created_at", "rowid"])
        .unwrap_or_else(|| "rowid".to_string());

    let sql = format!(
        "SELECT {}, {}, {} FROM messages WHERE thread_id = ?1 ORDER BY {} ASC LIMIT {LIMIT}",
        role_col,
        text_col,
        ts_col.as_deref().unwrap_or("NULL"),
        order_col,
    );
    let Ok(mut statement) = connection.prepare(&sql) else {
        return Vec::new();
    };
    let Ok(rows) = statement.query_map([thread], |row| {
        Ok((
            utils::text_cell(row, 0),
            utils::text_cell(row, 1),
            utils::text_cell(row, 2),
        ))
    }) else {
        return Vec::new();
    };

    let mut turns = Vec::new();
    for row in rows.flatten() {
        let (raw_role, raw_text, raw_ts) = row;
        let role = match raw_role.as_deref().unwrap_or("").to_lowercase().as_str() {
            "user" | "human" => "user",
            "assistant" | "ai" | "model" => "assistant",
            _ => continue,
        };
        let Some(raw) = raw_text else { continue };
        let timestamp = raw_ts
            .as_deref()
            .map(time::normalized_timestamp)
            .filter(|stamp| !stamp.is_empty());
        if let Some(turn) = transcript::build_turn(role, &payload_text(&raw), timestamp) {
            turns.push(turn);
        }
        if turns.len() >= LIMIT {
            break;
        }
    }
    turns
}

/// Text inside a `parts_json` payload: a plain string, a JSON string, an
/// array of `{type, text}` parts, or an object with a text-ish field.
/// Anything else falls back to the raw cell so a new shape still shows.
fn payload_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return trimmed.to_string();
    };
    text_in(&value).unwrap_or_else(|| trimmed.to_string())
}

fn text_in(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                if let Some(text) = text_in(item) {
                    if !text.trim().is_empty() {
                        out.push(text.trim().to_string());
                    }
                }
            }
            (!out.is_empty()).then(|| out.join("\n"))
        }
        serde_json::Value::Object(map) => {
            // Freebuff parts distinguish `kind: "text"` from `kind: "reasoning"`,
            // `kind: "tool"`, `kind: "ad"`. Tool calls and execution outputs
            // are internal actions, not conversation speech.
            if let Some(kind) = map
                .get("kind")
                .or_else(|| map.get("type"))
                .and_then(serde_json::Value::as_str)
            {
                if !matches!(kind, "text" | "input_text" | "output_text") {
                    return None;
                }
            }
            for key in ["text", "content", "message", "body"] {
                if let Some(found) = map.get(key).and_then(text_in) {
                    if !found.trim().is_empty() {
                        return Some(found);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// First candidate present in `columns`, so a release that renames one
/// still reads its threads.
fn pick_column(columns: &[String], candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .find(|name| columns.iter().any(|column| column == *name))
        .map(|name| (*name).to_string())
}
