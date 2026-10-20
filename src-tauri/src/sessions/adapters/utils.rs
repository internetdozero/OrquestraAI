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

use crate::sessions::types::TITLE_LIMIT;


/// Refuse to load a JSONL index bigger than this; such a file is a runaway
/// artifact, and parsing it would cost more than the listing is worth.
pub(crate) const MAX_JSONL_BYTES: u64 = 32 * 1024 * 1024;

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
        // `read_dir` follows a symlink. A session folder that *is* a link
        // would otherwise be walked, counting (and stalling on) the target.
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            _ => continue,
        }

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

/// Bytes of a regular file. `None` when the path is missing, a directory, or a
/// symlink: `metadata` and `read_to_string` follow links, so a session file
/// that is a link would be measured and loaded from outside the store. A link
/// also reports length 0, which would skip the size ceiling and read it whole.
pub(super) fn regular_file_len(path: &Path) -> Option<u64> {
    let metadata = fs::symlink_metadata(path).ok()?;
    metadata.file_type().is_file().then_some(metadata.len())
}

/// Size of one regular file, or `0` when it is missing or a symlink.
pub fn file_size(path: &Path) -> u64 {
    regular_file_len(path).unwrap_or(0)
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
    regular_file_len(path)?;
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Parse the first JSONL line without loading the whole transcript: rollout
/// files run to hundreds of megabytes and only the header matters.
pub fn first_json_line(path: &Path) -> Option<Value> {
    use std::io::BufRead;

    regular_file_len(path)?;
    let file = fs::File::open(path).ok()?;
    let mut line = String::new();
    std::io::BufReader::new(file).read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}

/// Every parsable line of a JSONL index. Oversized files are skipped whole.
pub fn read_jsonl(path: &Path) -> Vec<Value> {
    let Some(bytes) = regular_file_len(path) else {
        return Vec::new();
    };
    if bytes > MAX_JSONL_BYTES {
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
    // `is_file` follows symlinks. A store path that points at another database
    // would list rows from outside the harness.
    if regular_file_len(path).is_none() {
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

#[cfg(test)]
mod tests {
    use super::{dir_size, file_size};
    use std::fs;

    #[cfg(unix)]
    #[test]
    fn a_symlinked_session_tree_is_not_measured() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let scratch = std::path::PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-dir-size-{}-{unique}",
            std::process::id()
        ));
        let outside = scratch.join("outside");
        let session = scratch.join("vault").join("session");
        fs::create_dir_all(&outside).expect("outside tree");
        fs::create_dir_all(session.parent().expect("vault")).expect("vault");
        fs::write(outside.join("secret.bin"), vec![7u8; 4096]).expect("payload");
        fs::write(outside.join("note.txt"), b"note").expect("note");
        std::os::unix::fs::symlink(outside.join("note.txt"), outside.join("alias.txt"))
            .expect("file symlink");
        std::os::unix::fs::symlink(&outside, &session).expect("dir symlink");

        assert_eq!(dir_size(&outside), 4100, "real files still count");
        assert_eq!(dir_size(&session), 0, "the link itself is not walked");
        assert_eq!(
            crate::sessions::adapters::fsops::size_of(&session),
            0,
            "reclaim estimates use the same rule"
        );
        assert_eq!(file_size(&outside.join("secret.bin")), 4096);
        assert!(outside.join("secret.bin").is_file(), "the target stays put");

        fs::remove_file(&session).expect("unlink session");
        fs::remove_dir_all(&scratch).expect("cleanup");
    }
}
