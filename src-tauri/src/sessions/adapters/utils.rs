//! Shared helpers for the harness adapters.
//!
//! Three concerns live here so the adapters stay thin:
//! * **Sizing** — recursive byte counts that never follow a symlink, because
//!   harness trees contain links to projects outside the home directory.
//! * **Time** — see [`super::time`], re-exported below so adapters import one
//!   path for every helper.
//! * **SQLite** — every open is read-only with `query_only` set, so listing
//!   sessions can never block an agent that is mid-conversation.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, Row};
use serde_json::Value;

use crate::sessions::types::{TranscriptTurn, TITLE_LIMIT};

/// Most turns a transcript read returns; a full replay is the drawer's job
/// to page, not the core's to dump in one payload.
const MAX_TRANSCRIPT_TURNS: usize = 1000;
/// Longest single turn kept: tool dumps are trimmed, never dropped whole.
const MAX_TURN_CHARS: usize = 12_000;

/// Refuse to load a JSONL index bigger than this; such a file is a runaway
/// artifact, and parsing it would cost more than the listing is worth.
const MAX_JSONL_BYTES: u64 = 32 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Locations
// ---------------------------------------------------------------------------

/// The user's home directory. Adapters take it as a constructor argument so
/// tests can point them at a fixture tree instead.
pub fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set; cannot locate harness session stores".to_string())
}

// ---------------------------------------------------------------------------
// Sizing
// ---------------------------------------------------------------------------

/// Total bytes of every regular file under `path`, without following symlinks.
/// Unreadable entries are skipped: a lock file that vanishes mid-walk is not a
/// reason to report the whole harness as broken.
pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![path.to_path_buf()];

    while let Some(current) = stack.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };

            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    total += metadata.len();
                }
            }
            // Symlinks and special files are counted as zero on purpose.
        }
    }

    total
}

/// Size of one file, or `0` when it is missing.
pub fn file_size(path: &Path) -> u64 {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

/// Modification time in epoch seconds, used only when a store records no
/// timestamp of its own.
pub fn modified_secs(path: &Path) -> Option<i64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_secs() as i64)
}

// ---------------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------------

// Harness clock dialects live in `time`; re-exported so an adapter needs one
// import path for every helper it uses.
pub use super::time::{iso8601_from_secs, normalized_timestamp, now_secs, timestamp_secs};

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

/// Collapse whitespace and trim a prompt down to something a table cell can
/// show without breaking layout. Counts characters, not bytes, so a title is
/// never cut inside a multi-byte glyph.
pub fn trim_title(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= TITLE_LIMIT {
        return collapsed;
    }

    let mut trimmed: String = collapsed.chars().take(TITLE_LIMIT - 1).collect();
    trimmed.push('…');
    trimmed
}

/// Best effort recovery of a working directory from an encoded path segment
/// (`%2Fhome%2Fsystem`, `/home/system`) used by Grok's session folders.
pub fn decode_path_hint(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with('/') {
        return Some(raw.to_string());
    }
    if !raw.contains('%') {
        return None;
    }

    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok()
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

/// Read a JSON document, treating a missing or corrupt file as "nothing there".
pub fn read_json(path: &Path) -> Option<Value> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Parse the first JSONL line without loading the whole transcript: rollout
/// files run to hundreds of megabytes and only the header matters.
pub fn first_json_line(path: &Path) -> Option<Value> {
    use std::io::BufRead;

    let file = fs::File::open(path).ok()?;
    let mut line = String::new();
    std::io::BufReader::new(file).read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}

/// Every parsable line of a JSONL index. Oversized files are skipped whole.
pub fn read_jsonl(path: &Path) -> Vec<Value> {
    if file_size(path) > MAX_JSONL_BYTES {
        return Vec::new();
    }

    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };

    raw.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

/// First non-empty string among `keys`, so one helper covers `title`,
/// `thread_name`, `summary` and friends.
pub fn json_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(text) = value.get(*key).and_then(Value::as_str) else {
            continue;
        };
        let text = text.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }

    None
}

/// First parsable timestamp among `keys`, accepting numbers and strings.
pub fn json_timestamp(value: &Value, keys: &[&str]) -> Option<i64> {
    for key in keys {
        let Some(field) = value.get(*key) else {
            continue;
        };

        match field {
            Value::Number(number) => {
                if let Some(number) = number.as_f64() {
                    return Some(super::time::scale_epoch(number));
                }
            }
            Value::String(text) => {
                if let Some(secs) = timestamp_secs(text) {
                    return Some(secs);
                }
            }
            _ => {}
        }
    }

    None
}

/// Plain text of one transcript line's message, restricted to `text` blocks.
///
/// Tool calls and tool results are excluded on purpose: they are usually
/// command output or JSON arguments, not conversation. A corrupt or foreign
/// shape yields `None` instead of panicking.
fn turn_text(message: &Value) -> Option<String> {
    match message {
        Value::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        Value::Array(blocks) => {
            let mut joined = String::new();
            for block in blocks {
                let Some(block) = block.as_object() else {
                    continue;
                };
                let kind = block
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("text");
                if kind != "text" {
                    continue;
                }
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    let text = text.trim();
                    if !text.is_empty() {
                        if !joined.is_empty() {
                            joined.push('\n');
                        }
                        joined.push_str(text);
                    }
                }
            }
            (!joined.is_empty()).then_some(joined)
        }
        Value::Object(map) => map.get("content").and_then(turn_text),
        _ => None,
    }
}

/// Canonical role of one transcript line, or `None` to skip it.
///
/// Accepts the `role` field (`user`, `assistant`, `system`, plus the `human`
/// and `model` aliases some harnesses use) and the `type` marker Claude
/// writes on every line. Anything else — summaries, mode latches, permission
/// records — is not conversation and is skipped.
fn turn_role(entry: &Value) -> Option<&'static str> {
    for key in ["role", "type"] {
        let source = if key == "role" {
            entry.get("role").or_else(|| entry.get("message")?.get("role"))
        } else {
            entry.get("type")
        };
        if let Some(role) = source.and_then(Value::as_str) {
            return match role {
                "user" | "human" => Some("user"),
                "assistant" | "model" | "ai" => Some("assistant"),
                "system" => Some("system"),
                _ => continue,
            };
        }
    }
    None
}

/// Timestamp of one transcript line as ISO 8601 UTC, when recorded.
fn turn_timestamp(entry: &Value) -> Option<String> {
    if let Some(raw) = json_string(entry, &["timestamp", "published_at", "created_at"]) {
        return Some(normalized_timestamp(&raw));
    }
    json_timestamp(entry, &["timestamp", "published_at", "created_at"])
        .map(iso8601_from_secs)
}

/// One parsed turn from a raw JSON value, or `None` when the line carries no
/// conversation text.
fn parse_turn(entry: &Value) -> Option<TranscriptTurn> {
    let role = turn_role(entry)?;
    let message = entry.get("message").unwrap_or(entry);
    let mut content = turn_text(message)
        .or_else(|| entry.get("content").and_then(turn_text))
        .or_else(|| json_string(entry, &["text", "input", "output"]))?;
    if content.chars().count() > MAX_TURN_CHARS {
        content = content.chars().take(MAX_TURN_CHARS).collect();
        content.push('…');
    }
    Some(TranscriptTurn {
        role: role.to_string(),
        content,
        timestamp: turn_timestamp(entry),
    })
}

/// Every conversation turn in a JSONL transcript file.
///
/// Corrupt lines are skipped (`filter_map`, never `unwrap`): a harness that
/// is mid-write when the drawer opens must not break the read. A file that
/// parses as a single JSON array instead is read element-wise, so both
/// layouts work without the caller caring.
///
/// Oversized files are skipped whole, like [`read_jsonl`]: a runaway
/// artifact must not stall the UI thread.
///
/// [`read_jsonl`]: crate::sessions::adapters::utils::read_jsonl
pub fn read_transcript(path: &Path) -> Vec<TranscriptTurn> {
    if file_size(path) > MAX_JSONL_BYTES {
        return Vec::new();
    }

    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut turns: Vec<TranscriptTurn> = raw
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|entry| parse_turn(&entry))
        .take(MAX_TRANSCRIPT_TURNS)
        .collect();

    if turns.is_empty() {
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&raw) {
            turns = items
                .iter()
                .filter_map(parse_turn)
                .take(MAX_TRANSCRIPT_TURNS)
                .collect();
        }
    }

    turns
}

/// Human text of a message body: a plain string, a list of `{"text": ...}`
/// blocks, or an object with a `content` field holding either.
pub fn message_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        Value::Array(blocks) => {
            blocks
                .iter()
                .filter_map(message_text)
                .reduce(|mut joined, text| {
                    joined.push(' ');
                    joined.push_str(&text);
                    joined
                })
        }
        Value::Object(map) => map.get("content").and_then(message_text),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// SQLite
// ---------------------------------------------------------------------------

/// Open a harness database strictly read-only.
///
/// The URI flag plus `query_only` means a listing can run while the harness
/// holds the database open; SQLite refuses any write this connection attempts.
pub fn open_readonly(path: &Path) -> Result<Connection, String> {
    if !path.is_file() {
        return Err(format!("{} is missing", path.display()));
    }

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_URI;
    let connection = Connection::open_with_flags(path, flags)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(|error| format!("failed to set query_only on {}: {error}", path.display()))?;

    Ok(connection)
}

/// Does `table` exist in this database?
///
/// A missing table is `Ok(false)`. A schema that cannot be read at all — a
/// corrupt or truncated file, or one that is not SQLite — is an error, so a
/// broken store is reported instead of being mistaken for "no sessions".
pub fn table_exists(connection: &Connection, table: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |_| Ok(()),
        )
        .map(|()| true)
        .or_else(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => Ok(false),
            other => Err(format!("unreadable database schema: {other}")),
        })
}

/// Column names of `table`, empty when it is absent or not a plain table.
/// The name is validated rather than bound: SQLite cannot bind identifiers.
pub fn table_columns(connection: &Connection, table: &str) -> Vec<String> {
    if !table
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Vec::new();
    }

    let Ok(mut statement) = connection.prepare(&format!("PRAGMA table_info({table})")) else {
        return Vec::new();
    };
    let Ok(rows) = statement.query_map([], |row| row.get::<_, String>(1)) else {
        return Vec::new();
    };

    rows.flatten().collect()
}

/// Text of one cell, rendering numeric columns as text so adapters do not have
/// to know whether a harness stored a timestamp as INTEGER or TEXT.
pub fn text_cell(row: &Row, index: usize) -> Option<String> {
    match row.get_ref(index).ok()? {
        ValueRef::Text(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
        ValueRef::Integer(number) => Some(number.to_string()),
        ValueRef::Real(number) => Some(number.to_string()),
        _ => None,
    }
}

/// Integer value of one cell, accepting a numeric string as well.
pub fn i64_cell(row: &Row, index: usize) -> Option<i64> {
    match row.get_ref(index).ok()? {
        ValueRef::Integer(number) => Some(number),
        ValueRef::Real(number) => Some(number as i64),
        ValueRef::Text(bytes) => std::str::from_utf8(bytes).ok()?.trim().parse().ok(),
        _ => None,
    }
}
