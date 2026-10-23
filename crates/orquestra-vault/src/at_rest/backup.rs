//! Migration backup file: naming, plaintext recovery from it, purge,
//! and the startup sweep of leftovers.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use orquestra_core::paths::OrquestraPaths;

use super::envelope;
use super::io::zeroize_file_contents;
use super::unseal::try_open_with_available_keys;
use super::{AAD_MCPS, AAD_VAULT};

static CLEANUP_NOTICED: AtomicBool = AtomicBool::new(false);

/// Where the backup for `path` lives (`<file>.migrate-bak`).
pub(super) fn migrate_backup_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.migrate-bak",
        path.extension()
            .map(|extension| extension.to_string_lossy().into_owned())
            .unwrap_or_else(|| "json".to_string())
    ))
}

/// Read the backup: sealed backups open under the available keys first, a
/// plaintext backup is returned as-is. Only valid-JSON candidates qualify,
/// so the caller can schema-parse the result.
///
/// Nested envelopes (the double-seal incident) are unwrapped layer by
/// layer. A candidate that is still sealed afterwards is refused: handing
/// it back would make self-heal re-seal an envelope instead of the
/// catalog, growing the nesting instead of repairing it.
pub(super) fn recover_plaintext_from_backup(
    paths: &OrquestraPaths,
    path: &Path,
    aad: &[u8],
) -> Option<String> {
    let stored = std::fs::read_to_string(migrate_backup_path(path)).ok()?;
    if stored.trim().is_empty() {
        return None;
    }
    let mut candidate = match serde_json::from_str::<serde_json::Value>(&stored) {
        Ok(value) if envelope::is_sealed(&value) => {
            try_open_with_available_keys(paths, &value, aad).ok()?
        }
        _ => stored,
    };
    for _ in 0..8 {
        let value: serde_json::Value = serde_json::from_str(&candidate).ok()?;
        if !envelope::is_sealed(&value) {
            break;
        }
        candidate = try_open_with_available_keys(paths, &value, aad).ok()?;
    }
    let value: serde_json::Value = serde_json::from_str(&candidate).ok()?;
    if envelope::is_sealed(&value) {
        return None;
    }
    Some(candidate)
}

/// Overwrite `<path>.migrate-bak` with zeroes and unlink it, so a discarded
/// backup is not recoverable with file-carving tools.
///
/// Fail-closed ordering: the sealed file must still open under an available
/// key first — a backup is never destroyed while it might be the only
/// readable copy (see `recover_plaintext_from_backup`). Symlinks are refused
/// and a missing backup reports `Ok(false)`. Best effort on SSDs and
/// copy-on-write filesystems, where stale blocks may survive outside the
/// filesystem's reach: rotate exposed provider keys regardless (see
/// `docs/vault.md`).
pub fn purge_migration_backup(
    paths: &OrquestraPaths,
    path: &Path,
    aad: &[u8],
) -> Result<bool, String> {
    let backup = migrate_backup_path(path);
    let metadata = match std::fs::symlink_metadata(&backup) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!("failed to inspect {}: {error}", backup.display()));
        }
        Ok(metadata) => metadata,
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "refusing to purge {}: not a regular file",
            backup.display()
        ));
    }
    // Gate: the live file must open before its backup may die.
    let stored = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&stored)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    if !envelope::is_sealed(&value) {
        return Err(format!(
            "refusing to purge {}: sealed file is plaintext",
            backup.display()
        ));
    }
    try_open_with_available_keys(paths, &value, aad).map_err(|error| {
        format!(
            "refusing to purge {}: sealed file does not open ({error})",
            backup.display()
        )
    })?;
    zeroize_file_contents(&backup, metadata.len())?;
    std::fs::remove_file(&backup)
        .map_err(|error| format!("failed to remove {}: {error}", backup.display()))?;
    Ok(true)
}

/// Sweep plaintext `.migrate-bak` files left by alpha.5: once the sealed
/// file verifies under an available key, the readable backup is zeroized and
/// unlinked with a one-time notice. Sealed backups are left for the explicit
/// purge button; unverifiable sealed files keep their backup for recovery.
pub(super) fn startup_cleanup(paths: &OrquestraPaths) -> Result<bool, String> {
    let mut cleaned = false;
    for (name, aad) in [("vault.json", AAD_VAULT), ("mcps.json", AAD_MCPS)] {
        let path = paths.root.join(name);
        let backup = migrate_backup_path(&path);
        let Ok(meta) = std::fs::symlink_metadata(&backup) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_file() {
            continue;
        }
        let Ok(stored_backup) = std::fs::read_to_string(&backup) else {
            continue;
        };
        // Sealed backups are the new normal: the purge button owns them.
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&stored_backup) {
            if envelope::is_sealed(&value) {
                continue;
            }
        }
        let Ok(stored) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&stored) else {
            continue;
        };
        if !envelope::is_sealed(&value) {
            continue;
        }
        if try_open_with_available_keys(paths, &value, aad).is_err() {
            continue;
        }
        zeroize_file_contents(&backup, meta.len())?;
        std::fs::remove_file(&backup)
            .map_err(|error| format!("failed to remove {}: {error}", backup.display()))?;
        cleaned = true;
    }
    if cleaned && !CLEANUP_NOTICED.swap(true, Ordering::Relaxed) {
        crate::log_warn!(
            "removed alpha.5 plaintext migration backups after verifying the sealed files; \
             rotate any provider keys stored before the at-rest encryption migration"
        );
    }
    Ok(cleaned)
}
