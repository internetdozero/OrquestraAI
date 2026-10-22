//! One-way migration of legacy plaintext secrets, plus the two
//! self-heal paths (re-seal when a key shows up, downgrade when none).

use std::path::Path;

use orquestra_core::paths::OrquestraPaths;

use super::backup::{migrate_backup_path, recover_plaintext_from_backup};
use super::io::{persist_sealed, remove_backup_file, write_private_new};
use super::key;
use super::seal::seal_text;
use super::unseal::verify_reopen;

/// Migrate legacy plaintext: `0600` backup, atomic sealed write, reopen and
/// compare. On success the plaintext backup is unlinked and the migrated
/// flag is set, so no new plaintext backup lingers. On verification failure
/// the original bytes are restored and the readable backup is preserved.
pub(super) fn migrate_legacy_and_verify(
    paths: &OrquestraPaths,
    path: &Path,
    legacy: &str,
    aad: &[u8],
) -> Result<(), String> {
    let sealed = seal_text(paths, legacy, aad)?;
    let backup = migrate_backup_path(path);
    write_private_new(&backup, legacy.as_bytes())?;
    persist_sealed(path, &sealed)?;
    match verify_reopen(paths, path, aad, Some(legacy)) {
        Ok(()) => {
            if let Err(error) = remove_backup_file(&backup) {
                crate::log_warn!(
                    "migration verified but {} survived: {error}",
                    backup.display()
                );
            }
            key::mark_migrated(&paths.root)?;
            Ok(())
        }
        Err(error) => {
            // Keep both recovery paths intact: restore the original file and
            // leave the plaintext backup unchanged if verification failed.
            match persist_sealed(path, legacy) {
                Ok(()) => Err(format!(
                    "legacy migration verification failed for {}: {error}; original plaintext restored",
                    path.display()
                )),
                Err(restore_error) => Err(format!(
                    "legacy migration verification failed for {}: {error}; restoring the original also failed: {restore_error}",
                    path.display()
                )),
            }
        }
    }
}

/// Self-healing read for key rotation / corruption: fall back to the
/// `<file>.migrate-bak` backup (plaintext alpha.5 leftover or sealed) when
/// it holds valid JSON. The caller re-seals and rewrites atomically.
pub(super) fn self_heal(
    paths: &OrquestraPaths,
    path: &Path,
    aad: &[u8],
    keyring_err: Option<String>,
    fallback_err: String,
) -> Result<(String, bool), String> {
    let detail = keyring_err.map_or(fallback_err.clone(), |first| {
        format!("{first}; {fallback_err}")
    });
    let Some(recovered) = recover_plaintext_from_backup(paths, path, aad) else {
        return Err(format!("failed to unseal {}: {detail}", path.display()));
    };
    let backup = migrate_backup_path(path);
    crate::log_warn!(
        "secret file unsealed from {} after key mismatch ({detail}); re-sealed under the current file key",
        backup.display()
    );
    let sealed = seal_text(paths, &recovered, aad)?;
    persist_sealed(path, &sealed)?;
    Ok((recovered, false))
}

/// Same failure with no fallback key at all: only a backup can save us, and
/// re-sealing needs a key that `file_key` refuses to mint while sealed
/// files exist — so surface the unseal error unless a backup opens the way.
pub(super) fn self_heal_no_key(
    paths: &OrquestraPaths,
    path: &Path,
    aad: &[u8],
    err: String,
) -> Result<(String, bool), String> {
    let Some(recovered) = recover_plaintext_from_backup(paths, path, aad) else {
        return Err(format!("failed to unseal {}: {err}", path.display()));
    };
    // A backup exists but no key does: this is first-write territory only
    // when no sealed file strands us — `file_key` enforces that itself.
    match seal_text(paths, &recovered, aad) {
        Ok(sealed) => {
            persist_sealed(path, &sealed)?;
            Ok((recovered, false))
        }
        Err(_) => Err(format!("failed to unseal {}: {err}", path.display())),
    }
}
