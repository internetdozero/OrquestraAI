//! Destructive primitives shared by every adapter's `delete_session`.
//!
//! Everything here assumes a harness may be running at the same time: paths are
//! verified to sit inside the harness store before anything is unlinked,
//! symlinks are unlinked rather than followed, SQLite writes wait for a bounded
//! busy timeout instead of hanging, and index files are rewritten atomically so
//! a crash cannot truncate them.

use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use super::utils;

/// How long a write waits for a harness that currently holds a database.
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

// ---------------------------------------------------------------------------
// Filesystem
// ---------------------------------------------------------------------------

/// Bytes a path occupies, following the same rules as the listing.
pub fn size_of(path: &Path) -> u64 {
    if path.is_dir() {
        utils::dir_size(path)
    } else {
        utils::file_size(path)
    }
}

/// Refuse to touch anything outside the harness store it belongs to.
///
/// Adapters build paths by joining a store root, so this guards against a
/// session id that escapes — `../../elsewhere` or an absolute path — rather
/// than being a routine check. A `..` component is rejected outright: a path
/// like `<store>/../outside` still *starts with* the store, so a prefix test
/// alone would let it through.
pub fn ensure_within(root: &Path, target: &Path) -> Result<(), String> {
    let escapes = target
        .components()
        .any(|component| matches!(component, Component::ParentDir));
    let inside = target != root && target.starts_with(root) && !target.as_os_str().is_empty();

    if inside && !escapes {
        return Ok(());
    }

    Err(format!(
        "refusing to delete {}: outside {}",
        target.display(),
        root.display()
    ))
}

/// Remove a file, a directory tree or a symlink. Returns the bytes reclaimed,
/// or `0` when nothing was removed.
///
/// A symlink is unlinked, never followed: Freebuff worktrees link into
/// `/nsstyx/...` and removing the target by accident would be unrecoverable.
pub fn remove_path(root: &Path, target: &Path) -> Result<u64, String> {
    ensure_within(root, target)?;

    let Ok(metadata) = fs::symlink_metadata(target) else {
        // Already gone: purge is idempotent, and that is not a failure.
        return Ok(0);
    };

    if metadata.file_type().is_symlink() {
        return fs::remove_file(target)
            .map(|()| 0)
            .map_err(|error| format!("failed to unlink {}: {error}", target.display()));
    }

    let bytes = size_of(target);
    if metadata.is_dir() {
        fs::remove_dir_all(target)
            .map_err(|error| format!("failed to remove {}: {error}", target.display()))?;
    } else {
        fs::remove_file(target)
            .map_err(|error| format!("failed to remove {}: {error}", target.display()))?;
    }

    Ok(bytes)
}

/// Drop a directory that is now empty, stopping at `root`.
///
/// A directory still holding anything at all — including a non-empty
/// `memory/` — is left alone; deleting a harness's now-empty shell is a
/// courtesy, not a requirement.
/// An empty directory holds no bytes, so this reports whether it happened
/// rather than a size.
pub fn remove_if_empty(root: &Path, directory: &Path) -> bool {
    if directory == root || !directory.starts_with(root) || !directory.is_dir() {
        return false;
    }

    let Ok(mut entries) = fs::read_dir(directory) else {
        return false;
    };
    if entries.next().is_some() {
        return false;
    }

    fs::remove_dir(directory).is_ok()
}

/// Rewrite a JSONL index without the records `keep` rejects.
///
/// Lines that fail to parse are kept: a line this code cannot classify is not
/// evidence that it is safe to delete. Returns whether the file changed.
pub fn rewrite_jsonl(
    root: &Path,
    path: &Path,
    keep: &dyn Fn(&Value) -> bool,
) -> Result<bool, String> {
    ensure_within(root, path)?;
    if !path.is_file() {
        return Ok(false);
    }

    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    let mut dropped = 0_usize;
    let mut output = String::with_capacity(raw.len());

    for line in raw.lines() {
        match serde_json::from_str::<Value>(line) {
            Ok(record) if !keep(&record) => dropped += 1,
            _ => {
                output.push_str(line);
                output.push('\n');
            }
        }
    }

    if dropped == 0 {
        return Ok(false);
    }

    let temporary = path.with_file_name(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));

    let mut handle = fs::File::create(&temporary)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
    let written = handle
        .write_all(output.as_bytes())
        .and_then(|()| handle.sync_all());
    drop(handle);

    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(format!("failed to write {}: {error}", temporary.display()));
    }

    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to replace {}: {error}", path.display())
    })?;

    Ok(true)
}

/// Drop one key from a JSON object file, atomically.
///
/// Used for the registries a harness keeps of its open sessions: a stale key
/// there makes the CLI show a conversation whose store has been deleted. A
/// missing file, or a file that is not an object, is left untouched.
pub fn remove_json_key(root: &Path, path: &Path, key: &str) -> Result<bool, String> {
    ensure_within(root, path)?;
    let Some(Value::Object(mut map)) = utils::read_json(path) else {
        return Ok(false);
    };

    if map.remove(key).is_none() {
        return Ok(false);
    }

    let mut content = serde_json::to_string_pretty(&Value::Object(map))
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    content.push('\n');

    let temporary = path.with_file_name(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    fs::write(&temporary, content)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;

    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to replace {}: {error}", path.display())
    })?;

    Ok(true)
}

// ---------------------------------------------------------------------------
// SQLite
// ---------------------------------------------------------------------------

/// Open a harness database for writing, waiting a bounded time for a harness
/// that is mid-write instead of failing immediately or hanging forever.
pub fn open_writable(path: &Path) -> Result<Connection, String> {
    if !path.is_file() {
        return Err(format!("{} is missing", path.display()));
    }

    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags)
        .map_err(|error| format!("failed to open {} for writing: {error}", path.display()))?;
    connection.busy_timeout(BUSY_TIMEOUT).map_err(|error| {
        format!(
            "failed to set a busy timeout on {}: {error}",
            path.display()
        )
    })?;

    Ok(connection)
}

/// Delete rows matching one id, ignoring a table or column this release does
/// not have. Returns the number of rows removed.
pub fn delete_rows(
    connection: &Connection,
    table: &str,
    column: &str,
    id: &str,
) -> Result<usize, String> {
    if !utils::table_exists(connection, table)? {
        return Ok(0);
    }
    if !utils::table_columns(connection, table)
        .iter()
        .any(|name| name == column)
    {
        return Ok(0);
    }

    connection
        .execute(&format!("DELETE FROM {table} WHERE {column} = ?1"), [id])
        .map_err(|error| format!("DELETE FROM {table} failed: {error}"))
}

/// Enable foreign keys so a parent delete cascades to messages and parts.
pub fn enable_cascade(connection: &Connection) -> Result<(), String> {
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|error| format!("failed to enable foreign keys: {error}"))
}

/// Reclaim the pages a delete left behind.
///
/// A locked database is reported as an error rather than retried forever: the
/// rows are already gone, and the space can be reclaimed on the next run.
pub fn vacuum(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }

    let connection = open_writable(path)?;
    connection
        .execute_batch("VACUUM")
        .map_err(|error| format!("VACUUM {} failed: {error}", path.display()))
}

/// A database file plus the `-wal` and `-shm` sidecars SQLite keeps beside it.
pub fn sqlite_files(path: &Path) -> Vec<PathBuf> {
    let name = path.as_os_str().to_string_lossy().into_owned();

    [name.clone(), format!("{name}-wal"), format!("{name}-shm")]
        .into_iter()
        .map(PathBuf::from)
        .filter(|file| file.is_file())
        .collect()
}

/// Remove one harness-owned database and its sidecars, returning bytes freed.
pub fn remove_sqlite(root: &Path, path: &Path) -> Result<u64, String> {
    let mut bytes = 0;
    for file in sqlite_files(path) {
        bytes += remove_path(root, &file)?;
    }

    Ok(bytes)
}
