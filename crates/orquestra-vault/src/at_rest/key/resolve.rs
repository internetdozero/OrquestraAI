//! Key resolution for reads and writes: which key opens (or seals)
//! this root, promotion of the fallback key, and the migrated flag.

use std::path::Path;

use orquestra_core::paths::OrquestraPaths;

use crate::at_rest::envelope;

use super::fallback::{create_fallback_key, read_fallback_key};
use super::ring::{
    create_keyring_key, keyring_usable, migrated_flag, read_keyring_key, set_migrated_flag,
    store_keyring_key,
};
use super::{cache_get, cache_insert, warn_once};

const MIGRATED_FILE: &str = ".at-rest-migrated";

/// Any sealed secret file on disk? Gates fresh key generation: a new random
/// key while sealed files exist strands them (lockout).
fn any_sealed_file_exists(root: &Path) -> bool {
    for name in ["vault.json", "mcps.json"] {
        let path = root.join(name);
        let Ok(stored) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&stored) else {
            continue;
        };
        if envelope::is_sealed(&value) {
            return true;
        }
    }
    false
}

/// Write path: the key new seals use. Prefers the keyring key when present,
/// else the fallback key that already opens this root's files. Creates a
/// fresh key only when no sealed file exists yet (first write) — never as
/// a side effect of opening stranded files.
pub(crate) fn file_key(paths: &OrquestraPaths) -> Result<[u8; 32], String> {
    if let Some(hit) = cache_get(&paths.root) {
        return Ok(hit);
    }
    if keyring_usable(&paths.root) {
        match read_keyring_key() {
            Ok(Some(key)) => {
                cache_insert(&paths.root, key);
                return Ok(key);
            }
            Ok(None) => {}
            Err(error) => warn_once(format!(
                "OS keyring unreachable ({error}); using a 0600 key file — \
                 theft-of-disk protection is degraded until a keyring is available"
            )),
        }
        if let Some(key) = read_fallback_key(&paths.root)? {
            cache_insert(&paths.root, key);
            return Ok(key);
        }
        if any_sealed_file_exists(&paths.root) {
            return Err(
                "no file key: sealed files exist but neither the keyring nor the fallback key file holds a key"
                    .to_string(),
            );
        }
        let key = create_keyring_key()?;
        cache_insert(&paths.root, key);
        return Ok(key);
    }
    if let Some(key) = read_fallback_key(&paths.root)? {
        cache_insert(&paths.root, key);
        return Ok(key);
    }
    if any_sealed_file_exists(&paths.root) {
        return Err(
            "no file key: sealed files exist but no fallback key file holds a key".to_string(),
        );
    }
    let key = create_fallback_key(&paths.root)?;
    cache_insert(&paths.root, key);
    Ok(key)
}

/// Promote the fallback key into the keyring after it opened a sealed file:
/// copy its bytes (no new randomness — generation while sealed files exist
/// would strand them), creating the entry only when absent. When the keyring
/// already holds a *different* key, keep it: the caller re-seals the file
/// under that key instead.
pub(crate) fn promote_fallback_to_keyring(fallback_key: &[u8; 32]) -> Result<[u8; 32], String> {
    match read_keyring_key()? {
        Some(existing) => Ok(existing),
        None => {
            store_keyring_key(fallback_key)?;
            Ok(*fallback_key)
        }
    }
}

/// The flag lives next to the key material, never in the data files: a
/// keyring entry when the keyring is usable, a dotfile beside `.file-key`
/// otherwise. Either one counts, so SSH→desktop transitions keep it.
pub(crate) fn is_migrated(root: &Path) -> bool {
    if keyring_usable(root) && migrated_flag() {
        return true;
    }
    std::fs::read_to_string(root.join(MIGRATED_FILE))
        .map(|flag| flag.trim() == "1")
        .unwrap_or(false)
}

pub(crate) fn mark_migrated(root: &Path) -> Result<(), String> {
    if keyring_usable(root) {
        set_migrated_flag()?;
        return Ok(());
    }
    let path = root.join(MIGRATED_FILE);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
    }
    write_private_new(&path, b"1\n")
}

/// Create-only `0600` write for small key-adjacent files (no
/// write-then-chmod window). Overwrites an existing flag file in place.
fn write_private_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        if let Ok(file) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
        {
            use std::io::Write;
            let mut file = file;
            file.write_all(bytes)
                .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
            file.sync_all()
                .map_err(|error| format!("failed to sync {}: {error}", path.display()))?;
            return Ok(());
        }
    }
    std::fs::write(path, bytes)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to lock down {}: {error}", path.display()))?;
    }
    Ok(())
}
