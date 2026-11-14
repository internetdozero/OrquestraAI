//! Read-only SQLite access for harness stores. One open, three readers, all
//! tolerant of a schema a harness version has not created yet.

use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, Row};

use super::sizing::regular_file_len;

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
