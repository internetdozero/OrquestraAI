//! Encryption at rest for the secret-bearing files.
//!
//! `vault.json` and `mcps.json` carry API tokens, so they are sealed with
//! AES-256-GCM under a per-install file key (see [`key`]: OS keyring first,
//! `0600` key-file fallback on headless hosts). Every secret file is created
//! with mode `0600` — no write-then-chmod window — and writes stay atomic
//! (unique tmp + rename).
//!
//! Envelope versions: v1 (alpha.5, no AAD) still opens; every seal writes
//! v2, which binds the payload domain (`vault.v1` / `mcps.v1`) as AES-GCM
//! Additional Authenticated Data, so a `vault.json` envelope pasted into
//! `mcps.json` fails closed. Callers pass their domain and use
//! [`open_stored`], which migrates legacy plaintext (verify-then-unlink),
//! upgrades v1 → v2, and promotes a fallback key into the keyring.
//!
//! Once an install completes the encryption migration, [`key`]'s migrated
//! flag is set and plaintext secret files are refused instead of
//! re-sealed (downgrade protection). New migrations leave no plaintext
//! behind: the `<file>.migrate-bak` backup is unlinked once the sealed
//! file verifies, or overwritten sealed when verification fails. Plaintext
//! backups left by alpha.5 are swept by [`startup_cleanup`] once the sealed
//! file verifies — the operator still rotates the exposed keys (see
//! `CHANGELOG.md`) and purges sealed leftovers explicitly.

mod envelope;
mod key;

pub(crate) use envelope::{AAD_MCPS, AAD_VAULT};
// Test-only surface: production code reaches the flag via `key::is_migrated`.
#[cfg(test)]
pub(crate) use key::is_migrated;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::paths::OrquestraPaths;

static CLEANUP_NOTICED: AtomicBool = AtomicBool::new(false);

/// Seal JSON text for disk under the file key. The returned envelope is JSON
/// text, so the existing atomic writers are unchanged.
pub(crate) fn seal_text(
    paths: &OrquestraPaths,
    plaintext: &str,
    aad: &[u8],
) -> Result<String, String> {
    let key = key::file_key(paths)?;
    seal_with_key(&key, plaintext.as_bytes(), aad)
}

/// Seal with an explicit key (migration re-seals without re-resolving).
fn seal_with_key(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Result<String, String> {
    envelope::seal(key, plaintext, aad)
}

/// Open a stored file's text with the full migration flow (Task #323).
/// Returns the plaintext plus whether it was legacy input.
///
/// - Legacy JSON (or non-JSON) plaintext: refused once the migrated flag is
///   set; otherwise backed up `0600`, sealed, verified, and unlinked.
/// - Sealed v1: opened without AAD, then re-sealed as v2.
/// - Sealed v2: opened with the caller's AAD (swapped domains fail).
/// - Key fallback: the keyring key is tried first, then `.file-key` (reads
///   never create key material). A fallback success promotes its bytes into
///   the keyring and re-seals under the active key; `.file-key` is removed
///   only after the re-sealed files verify.
/// - Unseal failure: self-heals from the `.migrate-bak` backup (plaintext
///   or sealed) when it holds valid JSON, re-sealing under the current key.
pub(crate) fn open_stored(
    paths: &OrquestraPaths,
    path: &Path,
    stored: &str,
    aad: &[u8],
) -> Result<(String, bool), String> {
    let _ = startup_cleanup(paths);
    let value: serde_json::Value = match serde_json::from_str(stored) {
        Ok(value) => value,
        Err(_) => {
            if key::is_migrated(&paths.root) {
                return Err(refuse_plaintext(path));
            }
            return Ok((stored.to_string(), true));
        }
    };
    if !envelope::is_sealed(&value) {
        if key::is_migrated(&paths.root) {
            return Err(refuse_plaintext(path));
        }
        migrate_legacy_and_verify(paths, path, stored, aad)?;
        return Ok((stored.to_string(), true));
    }
    // Sealed: keyring first, then fallback. Neither read creates keys.
    let mut keyring_err: Option<String> = None;
    if key::keyring_usable(&paths.root) {
        match key::read_keyring_key() {
            Ok(Some(kp)) => match envelope::open(&kp, &value, aad) {
                Ok((plaintext, version)) => {
                    key::cache_insert(&paths.root, kp);
                    let text = String::from_utf8(plaintext)
                        .map_err(|error| format!("unsealed secrets are not valid text: {error}"))?;
                    if version == 1 {
                        let resealed = seal_with_key(&kp, text.as_bytes(), aad)?;
                        persist_sealed(path, &resealed)?;
                        verify_reopen(paths, path, aad, Some(&text))?;
                        key::mark_migrated(&paths.root)?;
                    } else {
                        let _ = maybe_remove_fallback(paths);
                    }
                    return Ok((text, false));
                }
                Err(error) => keyring_err = Some(error),
            },
            Ok(None) => {}
            Err(error) => {
                key::warn_once(format!(
                    "OS keyring unreachable ({error}); trying the fallback key file"
                ));
                keyring_err = Some(error);
            }
        }
    }
    match key::read_fallback_key(&paths.root)? {
        Some(kf) => match envelope::open(&kf, &value, aad) {
            Ok((plaintext, version)) => {
                let text = String::from_utf8(plaintext)
                    .map_err(|error| format!("unsealed secrets are not valid text: {error}"))?;
                if key::keyring_usable(&paths.root) {
                    let active = key::promote_fallback_to_keyring(&kf)?;
                    key::cache_insert(&paths.root, active);
                    let resealed = seal_with_key(&active, text.as_bytes(), aad)?;
                    persist_sealed(path, &resealed)?;
                    verify_reopen(paths, path, aad, Some(&text))?;
                    let _ = maybe_remove_fallback(paths);
                    key::mark_migrated(&paths.root)?;
                } else if version == 1 {
                    // Headless host with no keyring: no promotion target, but
                    // the v1 → v2 upgrade still applies under the same key.
                    key::cache_insert(&paths.root, kf);
                    let resealed = seal_with_key(&kf, text.as_bytes(), aad)?;
                    persist_sealed(path, &resealed)?;
                    verify_reopen(paths, path, aad, Some(&text))?;
                    key::mark_migrated(&paths.root)?;
                } else {
                    key::cache_insert(&paths.root, kf);
                }
                Ok((text, false))
            }
            Err(fallback_err) => self_heal(paths, path, aad, keyring_err, fallback_err),
        },
        None => {
            let err = keyring_err.unwrap_or_else(|| "no file key available".to_string());
            self_heal_no_key(paths, path, aad, err)
        }
    }
}

fn refuse_plaintext(path: &Path) -> String {
    format!(
        "refusing to read {}: plaintext secrets are no longer accepted after the \
         encryption migration completed — restore the sealed file or rotate the \
         exposed keys (see CHANGELOG.md)",
        path.display()
    )
}

/// Reopen `path` under the available keys (never creating any) and, when
/// `expected` is given, compare byte for byte.
fn verify_reopen(
    paths: &OrquestraPaths,
    path: &Path,
    aad: &[u8],
    expected: Option<&str>,
) -> Result<(), String> {
    let stored = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to re-read {}: {error}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&stored)
        .map_err(|error| format!("re-sealed {} does not parse: {error}", path.display()))?;
    if !envelope::is_sealed(&value) {
        return Err(format!(
            "re-sealed {} is not a sealed envelope",
            path.display()
        ));
    }
    let text = try_open_with_available_keys(paths, &value, aad)?;
    if let Some(want) = expected {
        if text != want {
            return Err(format!(
                "re-sealed {} does not match the migrated plaintext",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Try the keyring key, then the fallback key. Reads only — never creates.
fn try_open_with_available_keys(
    paths: &OrquestraPaths,
    envelope_value: &serde_json::Value,
    aad: &[u8],
) -> Result<String, String> {
    if key::keyring_usable(&paths.root) {
        if let Ok(Some(kp)) = key::read_keyring_key() {
            if let Ok((plaintext, _)) = envelope::open(&kp, envelope_value, aad) {
                return String::from_utf8(plaintext)
                    .map_err(|error| format!("unsealed secrets are not valid text: {error}"));
            }
        }
    }
    if let Some(kf) = key::read_fallback_key(&paths.root)? {
        if let Ok((plaintext, _)) = envelope::open(&kf, envelope_value, aad) {
            return String::from_utf8(plaintext)
                .map_err(|error| format!("unsealed secrets are not valid text: {error}"));
        }
    }
    Err("failed to unseal: wrong key or corrupted file".to_string())
}

/// Migrate legacy plaintext: `0600` backup, atomic sealed write, reopen and
/// compare. On success the plaintext backup is unlinked and the migrated
/// flag is set, so no new plaintext backup lingers. On verification failure
/// the backup is kept but overwritten sealed — never plaintext.
fn migrate_legacy_and_verify(
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
            // Never leave plaintext behind on a failed migration: keep the
            // backup, but sealed, so recovery stays possible without
            // readable secrets on disk.
            let _ = write_private_new(&backup, sealed.as_bytes());
            Err(format!(
                "legacy migration verification failed for {}: {error}",
                path.display()
            ))
        }
    }
}

/// Self-healing read for key rotation / corruption: fall back to the
/// `<file>.migrate-bak` backup (plaintext alpha.5 leftover or sealed) when
/// it holds valid JSON. The caller re-seals and rewrites atomically.
fn self_heal(
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
fn self_heal_no_key(
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

/// Where the backup for `path` lives (`<file>.migrate-bak`).
pub(crate) fn migrate_backup_path(path: &Path) -> PathBuf {
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
pub(crate) fn recover_plaintext_from_backup(
    paths: &OrquestraPaths,
    path: &Path,
    aad: &[u8],
) -> Option<String> {
    let stored = std::fs::read_to_string(migrate_backup_path(path)).ok()?;
    if stored.trim().is_empty() {
        return None;
    }
    let candidate = match serde_json::from_str::<serde_json::Value>(&stored) {
        Ok(value) if envelope::is_sealed(&value) => {
            try_open_with_available_keys(paths, &value, aad).ok()?
        }
        _ => stored,
    };
    serde_json::from_str::<serde_json::Value>(&candidate).ok()?;
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
pub(crate) fn purge_migration_backup(
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
pub(crate) fn startup_cleanup(paths: &OrquestraPaths) -> Result<bool, String> {
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

/// After a key promotion, drop `.file-key` once every existing sealed file
/// opens under the keyring key. Returns `true` when the fallback is gone.
fn maybe_remove_fallback(paths: &OrquestraPaths) -> Result<bool, String> {
    if !key::fallback_exists(&paths.root) || !key::keyring_usable(&paths.root) {
        return Ok(false);
    }
    let Ok(Some(kp)) = key::read_keyring_key() else {
        return Ok(false);
    };
    for (name, aad) in [("vault.json", AAD_VAULT), ("mcps.json", AAD_MCPS)] {
        let path = paths.root.join(name);
        let Ok(stored) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&stored) else {
            continue;
        };
        if !envelope::is_sealed(&value) {
            continue;
        }
        if envelope::open(&kp, &value, aad).is_err() {
            return Ok(false);
        }
    }
    key::remove_fallback_key(&paths.root)
}

/// Atomic sealed-file write: unique tmp created `0600`, fsync, same-dir
/// rename (no write-then-chmod window; the rename carries the tmp's mode).
fn persist_sealed(path: &Path, sealed: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
    }
    let tmp = path.with_extension(format!("json.tmp-{}", crate::paths::unique_tmp_suffix()));
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
fn write_private_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
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

fn remove_backup_file(backup: &Path) -> Result<(), String> {
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
        let (text, was_legacy) =
            open_stored(&paths, &paths.root.join("vault.json"), legacy, AAD_VAULT)
                .expect("open legacy");
        assert!(was_legacy);
        assert_eq!(text, legacy);
        // The migration sealed the file and left no plaintext backup behind.
        let target = paths.root.join("vault.json");
        let stored = std::fs::read_to_string(&target).expect("sealed file");
        let value: serde_json::Value = serde_json::from_str(&stored).expect("envelope");
        assert!(envelope::is_sealed(&value));
        assert_eq!(envelope::sealed_version(&value), Some(2));
        assert!(!migrate_backup_path(&target).exists());
        let (reopened, was_legacy) =
            open_stored(&paths, &target, &stored, AAD_VAULT).expect("open sealed");
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
        let sealed = seal_text(&paths, legacy, AAD_VAULT).expect("seal");
        persist_sealed(&target, &sealed).expect("write sealed");
        // One flipped ciphertext byte fails closed under the same key.
        let mut value: serde_json::Value = serde_json::from_str(&sealed).expect("envelope");
        let mut raw = BASE64_STANDARD
            .decode(value.get("data").unwrap().as_str().unwrap())
            .expect("data");
        raw[0] ^= 0x01;
        value["data"] = serde_json::Value::String(BASE64_STANDARD.encode(raw));
        let corrupted = serde_json::to_string_pretty(&value).unwrap();
        persist_sealed(&target, &corrupted).expect("corrupt");
        assert!(
            open_stored(&paths, &target, &corrupted, AAD_VAULT).is_err(),
            "tampered seal must fail"
        );
        // No backup yet: nothing to heal from.
        assert!(recover_plaintext_from_backup(&paths, &target, AAD_VAULT).is_none());
        write_private_new(&migrate_backup_path(&target), legacy.as_bytes()).expect("plant backup");
        assert_eq!(
            recover_plaintext_from_backup(&paths, &target, AAD_VAULT).as_deref(),
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
        let sealed = seal_text(&paths, legacy, AAD_VAULT).expect("seal");
        persist_sealed(&target, &sealed).expect("write sealed");
        let backup = migrate_backup_path(&target);
        write_private_new(&backup, legacy.as_bytes()).expect("plant backup");
        assert!(purge_migration_backup(&paths, &target, AAD_VAULT).expect("purge"));
        assert!(!backup.exists(), "backup must be unlinked");
        assert!(
            !purge_migration_backup(&paths, &target, AAD_VAULT).expect("re-purge"),
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
        let sealed = seal_text(&paths, legacy, AAD_VAULT).expect("seal");
        let mut value: serde_json::Value = serde_json::from_str(&sealed).expect("envelope");
        let mut raw = BASE64_STANDARD
            .decode(value.get("data").unwrap().as_str().unwrap())
            .expect("data");
        raw[0] ^= 0x01;
        value["data"] = serde_json::Value::String(BASE64_STANDARD.encode(raw));
        persist_sealed(&target, &serde_json::to_string_pretty(&value).unwrap())
            .expect("corrupt sealed");
        let backup = migrate_backup_path(&target);
        write_private_new(&backup, legacy.as_bytes()).expect("plant backup");
        assert!(purge_migration_backup(&paths, &target, AAD_VAULT).is_err());
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

    /// Task #323: once the migrated flag is set, plaintext is refused — and
    /// every secret file is created `0600`.
    #[test]
    fn legacy_plaintext_is_refused_once_the_flag_is_set() {
        let paths = test_paths("flag-refuse");
        let target = paths.root.join("vault.json");
        let legacy = r#"{"services":{"github":{"keys":[{"secret":"sk-hidden"}]}}}"#;
        let (text, was_legacy) = open_stored(&paths, &target, legacy, AAD_VAULT).expect("migrate");
        assert!(was_legacy);
        assert_eq!(text, legacy);
        assert!(key::is_migrated(&paths.root), "migration sets the flag");
        assert!(!migrate_backup_path(&target).exists(), "backup unlinked");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        // A planted plaintext file is now a downgrade: refused, not re-sealed.
        let stored = std::fs::read_to_string(&target).expect("sealed stands");
        std::fs::write(&target, legacy).expect("plant plaintext");
        let err = open_stored(&paths, &target, legacy, AAD_VAULT).expect_err("must refuse");
        assert!(err.contains("no longer accepted"), "clear refusal: {err}");
        // The sealed file is restored for the next load.
        persist_sealed(&target, &stored).expect("restore sealed");
        let (reopened, was_legacy) =
            open_stored(&paths, &target, &stored, AAD_VAULT).expect("open sealed");
        assert!(!was_legacy);
        assert_eq!(reopened, legacy);
        let _ = std::fs::remove_dir_all(&paths.root);
    }

    /// Task #323: sealed v1 (alpha.5, no AAD) opens and is re-sealed as v2.
    #[test]
    fn v1_envelopes_upgrade_to_v2_on_load() {
        let paths = test_paths("v1-upgrade");
        let target = paths.root.join("vault.json");
        let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
        let key = key::file_key(&paths).expect("file key");
        let v1 = envelope::seal_v1_for_tests(&key, legacy.as_bytes()).expect("v1 seal");
        persist_sealed(&target, &v1).expect("plant v1");
        let stored = std::fs::read_to_string(&target).expect("read v1");
        let before: serde_json::Value = serde_json::from_str(&stored).expect("v1 JSON");
        assert_eq!(envelope::sealed_version(&before), Some(1));
        let (text, was_legacy) = open_stored(&paths, &target, &stored, AAD_VAULT).expect("upgrade");
        assert!(!was_legacy);
        assert_eq!(text, legacy);
        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&target).expect("re-read"))
                .expect("v2 JSON");
        assert_eq!(envelope::sealed_version(&after), Some(2));
        let (again, _) = open_stored(
            &paths,
            &target,
            &std::fs::read_to_string(&target).expect("read v2"),
            AAD_VAULT,
        )
        .expect("v2 opens");
        assert_eq!(again, legacy);
        let _ = std::fs::remove_dir_all(&paths.root);
    }

    /// Task #323: SSH → desktop handoff — a vault sealed under the fallback
    /// file key promotes its bytes into the (mock) keyring on first unseal,
    /// re-seals under the active key, and drops `.file-key` only after the
    /// re-sealed file verifies.
    #[test]
    fn fallback_key_migrates_to_keyring_on_unseal() {
        use base64::prelude::*;
        let _serial = key::test_keyring_lock();
        key::test_keyring_clear();
        // A non-temp root takes the keyring path (temp roots stay pinned to
        // the fallback file so suites never touch a shared key namespace).
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "orquestra-at-rest-handoff-{}-{nanos}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch");
        let paths = OrquestraPaths {
            root: root.clone(),
            vault: root.clone(),
            active: root.clone(),
        };
        assert!(
            key::keyring_usable(&paths.root),
            "mock keyring must be usable"
        );
        let target = paths.root.join("vault.json");
        let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
        let fallback: [u8; 32] = [0x77; 32];
        // Plant the SSH-era state: fallback file + file sealed under it.
        write_private_new(
            &paths.root.join(".file-key"),
            format!("{}\n", BASE64_STANDARD.encode(fallback)).as_bytes(),
        )
        .expect("plant fallback");
        let sealed = seal_with_key(&fallback, legacy.as_bytes(), AAD_VAULT).expect("seal");
        persist_sealed(&target, &sealed).expect("plant sealed");
        // First unseal promotes, re-seals, and removes the fallback file.
        let stored = std::fs::read_to_string(&target).expect("read sealed");
        let (text, was_legacy) = open_stored(&paths, &target, &stored, AAD_VAULT).expect("unseal");
        assert!(!was_legacy);
        assert_eq!(text, legacy);
        let raw = key::test_keyring_raw().expect("read mock");
        assert_eq!(
            raw.as_deref(),
            Some(BASE64_STANDARD.encode(fallback).as_str()),
            "fallback bytes move into the keyring"
        );
        assert!(
            !paths.root.join(".file-key").exists(),
            ".file-key is removed after the re-sealed file verifies"
        );
        let reopened: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&target).expect("re-read"))
                .expect("envelope");
        assert_eq!(envelope::sealed_version(&reopened), Some(2));
        key::test_keyring_clear();
        let _ = std::fs::remove_dir_all(&paths.root);
    }

    /// Task #323: `startup_cleanup` removes alpha.5 plaintext backups once
    /// the sealed file verifies, and leaves sealed backups for the purge
    /// button — as well as backups whose sealed file does not open.
    #[test]
    fn startup_cleanup_sweeps_only_verified_plaintext_backups() {
        let paths = test_paths("startup-sweep");
        let target = paths.root.join("vault.json");
        let legacy = r#"{"services":{"svc":{"id":"svc","label":"S","keys":[]}}}"#;
        // Verified pair: plaintext backup goes away.
        let sealed = seal_text(&paths, legacy, AAD_VAULT).expect("seal");
        persist_sealed(&target, &sealed).expect("write sealed");
        let backup = migrate_backup_path(&target);
        write_private_new(&backup, legacy.as_bytes()).expect("plant plaintext backup");
        assert!(startup_cleanup(&paths).expect("sweep"));
        assert!(!backup.exists());
        // A sealed backup is left for the explicit purge button.
        write_private_new(&backup, sealed.as_bytes()).expect("plant sealed backup");
        assert!(!startup_cleanup(&paths).expect("sweep sealed"));
        assert!(backup.exists());
        let _ = std::fs::remove_file(&backup);
        // An unverifiable sealed file keeps its plaintext backup: it may be
        // the only readable copy.
        use base64::prelude::*;
        let mut value: serde_json::Value = serde_json::from_str(&sealed).expect("envelope");
        let mut raw = BASE64_STANDARD
            .decode(value.get("data").unwrap().as_str().unwrap())
            .expect("data");
        raw[0] ^= 0x01;
        value["data"] = serde_json::Value::String(BASE64_STANDARD.encode(raw));
        persist_sealed(&target, &serde_json::to_string_pretty(&value).unwrap())
            .expect("corrupt sealed");
        write_private_new(&backup, legacy.as_bytes()).expect("plant backup");
        assert!(!startup_cleanup(&paths).expect("sweep corrupt"));
        assert!(backup.exists(), "unverifiable backup survives");
        let _ = std::fs::remove_dir_all(&paths.root);
    }
}
