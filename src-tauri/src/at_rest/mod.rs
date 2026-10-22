//! Encryption at rest for the secret-bearing files.
//!
//! `vault.json` and `mcps.json` carry API tokens, so they are sealed with
//! AES-256-GCM under a per-install file key. The key lives in the OS
//! keyring (Secret Service / Keychain / Credential Manager); headless hosts
//! without a keyring fall back to a `0600` key file with a loud warning.
//! POSIX `0600` on the files themselves stays as defense in depth.
//!
//! Format detection keeps the rollout reversible: [`open_text`] accepts both
//! sealed envelopes and legacy plaintext. On a legacy read the caller backs
//! the plaintext up (`<file>.migrate-bak`) and rewrites it sealed via
//! [`migrate_legacy_file`], so one load migrates the file and dual-read
//! covers crashes in between.

mod envelope;
mod key;

use std::path::{Path, PathBuf};

use crate::paths::OrquestraPaths;

/// Seal JSON text for disk. The returned envelope is JSON text, so the
/// existing atomic writers (`write_private`, `write_config`) are unchanged.
pub(crate) fn seal_text(paths: &OrquestraPaths, plaintext: &str) -> Result<String, String> {
    let key = key::file_key(paths)?;
    envelope::seal(&key, plaintext.as_bytes())
}

/// Open stored file text. Returns the plaintext plus whether it was legacy
/// (unsealed) input that the caller should migrate.
pub(crate) fn open_text(paths: &OrquestraPaths, stored: &str) -> Result<(String, bool), String> {
    let value: serde_json::Value = match serde_json::from_str(stored) {
        Ok(value) => value,
        // Not JSON at all: leave it legacy so the caller's parser reports
        // the corruption exactly as before.
        Err(_) => return Ok((stored.to_string(), true)),
    };
    if !envelope::is_sealed(&value) {
        return Ok((stored.to_string(), true));
    }
    let key = key::file_key(paths)?;
    let plaintext = envelope::open(&key, &value)?;
    String::from_utf8(plaintext)
        .map_err(|error| format!("unsealed secrets are not valid text: {error}"))
        .map(|text| (text, false))
}

/// Where the plaintext backup for `path` lives (`<file>.migrate-bak`).
pub(crate) fn migrate_backup_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.migrate-bak",
        path.extension()
            .map(|extension| extension.to_string_lossy().into_owned())
            .unwrap_or_else(|| "json".to_string())
    ))
}

/// Self-healing read for key rotation / corruption: when the sealed file no
/// longer opens under the current file key, fall back to the
/// `<file>.migrate-bak` plaintext backup left by the encryption migration.
/// Returns the backup plaintext when it holds valid JSON (a sealed backup
/// is opened under the current key first); the caller validates it against
/// its own schema, re-seals it, and rewrites the file atomically.
pub(crate) fn recover_plaintext_from_backup(
    paths: &OrquestraPaths,
    path: &Path,
) -> Option<String> {
    let stored = std::fs::read_to_string(migrate_backup_path(path)).ok()?;
    if stored.trim().is_empty() {
        return None;
    }
    let candidate = match serde_json::from_str::<serde_json::Value>(&stored) {
        Ok(value) if envelope::is_sealed(&value) => open_text(paths, &stored).ok()?.0,
        _ => stored,
    };
    serde_json::from_str::<serde_json::Value>(&candidate).ok()?;
    Some(candidate)
}

/// Overwrite `<path>.migrate-bak` with zeroes and unlink it, so a discarded
/// plaintext backup is not recoverable with file-carving tools.
///
/// Fail-closed ordering: the sealed file must still open under the current
/// file key first — a backup is never destroyed while it might be the only
/// readable copy (see `recover_plaintext_from_backup`). Symlinks are refused
/// and a missing backup reports `Ok(false)`. Best effort on SSDs and
/// copy-on-write filesystems, where stale blocks may survive outside the
/// filesystem's reach: rotate exposed provider keys regardless (see
/// `docs/vault.md`).
pub(crate) fn purge_migration_backup(paths: &OrquestraPaths, path: &Path) -> Result<bool, String> {
    let backup = migrate_backup_path(path);
    let metadata = match std::fs::symlink_metadata(&backup) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to inspect {}: {error}",
                backup.display()
            ));
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
    open_text(paths, &stored).map_err(|error| {
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

/// Overwrite a file's bytes with zeroes without changing its length, then
/// sync, so the plaintext is gone before the directory entry is unlinked.
fn zeroize_file_contents(path: &Path, len: u64) -> Result<(), String> {
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

/// Back up legacy `path` to `<path>.migrate-bak` and return the sealed
/// replacement. The caller persists the sealed text with its own atomic
/// writer; if that write fails the original is untouched and still loads.
pub(crate) fn migrate_legacy_file(
    path: &Path,
    legacy: &str,
    paths: &OrquestraPaths,
) -> Result<String, String> {
    let sealed = seal_text(paths, legacy)?;
    let backup = migrate_backup_path(path);
    std::fs::write(&backup, legacy)
        .map_err(|error| format!("failed to back up {}: {error}", backup.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&backup, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to lock down {}: {error}", backup.display()))?;
    }
    Ok(sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(name: &str) -> OrquestraPaths {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "orquestra-migrate-{name}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("scratch");
        OrquestraPaths {
            root: root.clone(),
            vault: root.clone(),
            active: root,
        }
    }

    #[test]
    fn sealed_text_opens_clean_and_legacy_is_flagged() {
        let paths = test_paths("flag");
        let legacy = r#"{"services":{"a":{"keys":[]}}}"#;
        let (text, was_legacy) = open_text(&paths, legacy).expect("open legacy");
        assert!(was_legacy);
        assert_eq!(text, legacy);
        let sealed = seal_text(&paths, legacy).expect("seal");
        let (reopened, was_legacy) = open_text(&paths, &sealed).expect("open sealed");
        assert!(!was_legacy);
        assert_eq!(reopened, legacy);
        let _ = std::fs::remove_dir_all(&paths.root);
    }

    /// Task #314: a sealed file the current key no longer opens (rotation,
    /// restored backup, flipped byte) recovers from `.migrate-bak`.
    #[test]
    fn corrupt_sealed_file_recovers_from_migrate_bak() {
        use base64::prelude::*;
        let paths = test_paths("selfheal");
        let target = paths.root.join("vault.json");
        let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
        let sealed = seal_text(&paths, legacy).expect("seal");
        std::fs::write(&target, &sealed).expect("write sealed");
        // One flipped ciphertext byte fails closed under the same key.
        let mut value: serde_json::Value = serde_json::from_str(&sealed).expect("envelope");
        let mut raw = BASE64_STANDARD
            .decode(value.get("data").unwrap().as_str().unwrap())
            .expect("data");
        raw[0] ^= 0x01;
        value["data"] = serde_json::Value::String(BASE64_STANDARD.encode(raw));
        std::fs::write(&target, serde_json::to_string_pretty(&value).unwrap())
            .expect("corrupt");
        let stored = std::fs::read_to_string(&target).expect("read corrupted");
        assert!(open_text(&paths, &stored).is_err(), "tampered seal must fail");
        // No backup yet: nothing to heal from.
        assert!(recover_plaintext_from_backup(&paths, &target).is_none());
        std::fs::write(migrate_backup_path(&target), legacy).expect("plant backup");
        assert_eq!(
            recover_plaintext_from_backup(&paths, &target).as_deref(),
            Some(legacy)
        );
        let _ = std::fs::remove_dir_all(&paths.root);
    }

    /// Task #320: purge zeroizes then unlinks; a missing backup is `Ok(false)`.
    #[test]
    fn purge_zeroizes_then_removes_the_backup() {
        let paths = test_paths("purge");
        let target = paths.root.join("vault.json");
        let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
        let sealed = seal_text(&paths, legacy).expect("seal");
        std::fs::write(&target, &sealed).expect("write sealed");
        let backup = migrate_backup_path(&target);
        std::fs::write(&backup, legacy).expect("plant backup");
        assert!(purge_migration_backup(&paths, &target).expect("purge"));
        assert!(!backup.exists(), "backup must be unlinked");
        assert!(
            !purge_migration_backup(&paths, &target).expect("re-purge"),
            "absent backup reports Ok(false)"
        );
        let _ = std::fs::remove_dir_all(&paths.root);
    }

    /// Task #320: while the sealed file does not open, the backup — possibly
    /// the only readable copy — survives the refused purge.
    #[test]
    fn purge_refuses_while_the_sealed_file_does_not_open() {
        use base64::prelude::*;
        let paths = test_paths("purge-refuse");
        let target = paths.root.join("vault.json");
        let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
        let sealed = seal_text(&paths, legacy).expect("seal");
        let mut value: serde_json::Value = serde_json::from_str(&sealed).expect("envelope");
        let mut raw = BASE64_STANDARD
            .decode(value.get("data").unwrap().as_str().unwrap())
            .expect("data");
        raw[0] ^= 0x01;
        value["data"] = serde_json::Value::String(BASE64_STANDARD.encode(raw));
        std::fs::write(&target, serde_json::to_string_pretty(&value).unwrap())
            .expect("corrupt sealed");
        let backup = migrate_backup_path(&target);
        std::fs::write(&backup, legacy).expect("plant backup");
        assert!(purge_migration_backup(&paths, &target).is_err());
        assert_eq!(
            std::fs::read_to_string(&backup).expect("backup survives"),
            legacy
        );
        let _ = std::fs::remove_dir_all(&paths.root);
    }

    /// Task #320: the zeroize helper blanks every byte past the 64KB chunk
    /// boundary without changing the file length.
    #[test]
    fn zeroize_helper_blanks_every_byte() {
        let paths = test_paths("purge-zero");
        let file = paths.root.join("scratch.bin");
        let original: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&file, &original).expect("write scratch");
        let len = std::fs::metadata(&file).expect("meta").len();
        zeroize_file_contents(&file, len).expect("zeroize");
        let blanked = std::fs::read(&file).expect("read blanked");
        assert_eq!(blanked.len(), original.len());
        assert!(blanked.iter().all(|byte| *byte == 0));
        let _ = std::fs::remove_dir_all(&paths.root);
    }

    #[test]
    fn migration_keeps_a_plaintext_backup_and_round_trips() {
        let paths = test_paths("migrate");
        let target = paths.root.join("vault.json");
        let legacy = r#"{"services":{"github":{"keys":[{"secret":"sk-hidden"}]}}}"#;
        let sealed = migrate_legacy_file(&target, legacy, &paths).expect("migrate");
        assert!(!sealed.contains("sk-hidden"));
        let backup = paths.root.join("vault.json.migrate-bak");
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), legacy);
        let (reopened, was_legacy) = open_text(&paths, &sealed).expect("open");
        assert!(!was_legacy);
        assert_eq!(reopened, legacy);
        let _ = std::fs::remove_dir_all(&paths.root);
    }
}
