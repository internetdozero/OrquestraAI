//! File primitives shared by the seal/unseal paths: durable sealed
//! writes, `0600` private writes, backup removal and zeroize.

use std::path::Path;

/// Atomic sealed-file write: unique tmp created `0600`, fsync, same-dir
/// rename (no write-then-chmod window; the rename carries the tmp's mode).
pub(super) fn persist_sealed(path: &Path, sealed: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
    }
    let tmp = path.with_extension(format!(
        "json.tmp-{}",
        orquestra_core::paths::unique_tmp_suffix()
    ));
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&tmp)
            .map_err(|error| format!("failed to write {}: {error}", tmp.display()))?;
        file.write_all(sealed.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", tmp.display()))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", tmp.display()))?;
    }
    std::fs::rename(&tmp, path)
        .map_err(|error| format!("failed to replace {}: {error}", path.display()))?;
    Ok(())
}

/// Create-only `0600` write for small sidecar files (backups, flags) — no
/// write-then-chmod window. Truncates an existing file in place.
pub(super) fn write_private_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
    }
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(path)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        file.write_all(bytes)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", path.display()))?;
    }
    Ok(())
}

pub(super) fn remove_backup_file(backup: &Path) -> Result<(), String> {
    let len = std::fs::metadata(backup)
        .map_err(|error| format!("failed to inspect {}: {error}", backup.display()))?
        .len();
    zeroize_file_contents(backup, len)?;
    std::fs::remove_file(backup)
        .map_err(|error| format!("failed to remove {}: {error}", backup.display()))?;
    Ok(())
}

/// Overwrite a file's bytes with zeroes without changing its length, then
/// sync, so the secret is gone before the directory entry is unlinked.
pub(super) fn zeroize_file_contents(path: &Path, len: u64) -> Result<(), String> {
    use std::io::{Seek, SeekFrom, Write};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|error| format!("failed to open {} for purge: {error}", path.display()))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("failed to rewind {} for purge: {error}", path.display()))?;
    let zeroes = [0u8; 65536];
    let mut remaining = len;
    while remaining > 0 {
        let chunk = remaining.min(zeroes.len() as u64) as usize;
        file.write_all(&zeroes[..chunk])
            .map_err(|error| format!("failed to purge {}: {error}", path.display()))?;
        remaining -= chunk as u64;
    }
    file.sync_all()
        .map_err(|error| format!("failed to sync {} for purge: {error}", path.display()))?;
    Ok(())
}
