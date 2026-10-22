//! File-key resolution: OS keyring first, 0600 file fallback.
//!
//! One random 256-bit key encrypts both secret files (`vault.json`,
//! `mcps.json`). The key lives in the OS keyring (Secret Service on Linux,
//! Keychain on macOS, Credential Manager on Windows). When no keyring is
//! reachable — headless hosts, SSH sessions, CI, locked collections — the
//! backend falls back to a `0600` key file beside the secrets and warns
//! loudly once per process, so the degraded mode is explicit rather
//! than silent.
//!
//! Dual-source rule (Task #323): reads never create key material. When a
//! sealed file exists but the keyring is empty, resolving must NOT generate
//! a fresh keyring key — that strands the sealed files (lockout). Instead
//! the unseal path tries the keyring key first, then `.file-key`; when the
//! fallback opens the file its bytes are promoted into the keyring (no new
//! randomness) and the file is re-sealed, with `.file-key` removed only
//! after the re-sealed files verify.
//!
//! Resolved keys are cached in memory per storage root, so the keyring (or
//! the fallback file) is touched once per process, not per request.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, OnceLock,
};

use base64::prelude::*;

use orquestra_core::paths::OrquestraPaths;

use super::envelope;

const SERVICE: &str = "orquestra-ai";
const ACCOUNT: &str = "at-rest-file-key";
const MIGRATED_ACCOUNT: &str = "at-rest-migrated";
const FALLBACK_FILE: &str = ".file-key";
const MIGRATED_FILE: &str = ".at-rest-migrated";

static CACHE: OnceLock<Mutex<HashMap<String, [u8; 32]>>> = OnceLock::new();
static WARNED: AtomicBool = AtomicBool::new(false);

fn cache() -> &'static Mutex<HashMap<String, [u8; 32]>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn cache_insert(root: &Path, key: [u8; 32]) {
    let scope = root.to_string_lossy().into_owned();
    if let Ok(mut guard) = cache().lock() {
        guard.insert(scope, key);
    }
}

fn cache_get(root: &Path) -> Option<[u8; 32]> {
    cache()
        .lock()
        .ok()
        .and_then(|guard| guard.get(&root.to_string_lossy().into_owned()).copied())
}

/// True when the OS keyring must not be touched: integration-test suites
/// set `ORQUESTRA_KEYRING_DISABLE=1` (see `tests/common`), and any storage
/// root under the OS temp dir is a scratch tree by construction — the real
/// `~/.orquestra` never lives there. Both keep hermetic suites off the
/// operator's real keyring entry, which is global per service/account and
/// would otherwise be read (or created) by every test process.
fn keyring_disabled(root: &Path) -> bool {
    if std::env::var("ORQUESTRA_KEYRING_DISABLE")
        .map(|flag| flag == "1" || flag.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return true;
    }
    #[cfg(test)]
    {
        // Unit tests run against a hermetic in-memory keyring mock (see
        // `keyring_backend` below), never the operator's real keyring —
        // except scratch roots under the temp dir, which stay pinned to
        // the 0600 fallback file so per-root isolation holds.
        if root.starts_with(std::env::temp_dir()) {
            return true;
        }
        return false;
    }
    #[cfg(not(test))]
    {
        root.starts_with(std::env::temp_dir())
    }
}

/// Whether the keyring backend is reachable right now. Native backends need
/// no D-Bus. On Linux, skip the keyring fast when no session bus exists
/// instead of waiting out a secret-service timeout.
fn keyring_reachable() -> bool {
    #[cfg(test)]
    {
        true
    }
    #[cfg(not(test))]
    {
        #[cfg(target_os = "linux")]
        {
            if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
                return true;
            }
            // SAFETY: `getuid` takes no arguments and has no failure mode.
            let uid = unsafe { libc::getuid() };
            Path::new(&format!("/run/user/{uid}/bus")).exists()
        }
        #[cfg(not(target_os = "linux"))]
        {
            true
        }
    }
}

pub(crate) fn keyring_usable(root: &Path) -> bool {
    !keyring_disabled(root) && keyring_reachable()
}

// ---------------------------------------------------------------------------
// Keyring backend (real in production, hermetic mock under `cfg(test)`)
// ---------------------------------------------------------------------------

#[cfg(not(test))]
mod keyring_backend {
    pub(super) fn get(account: &str) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(super::SERVICE, account)
            .map_err(|error| format!("could not open the OS keyring entry: {error}"))?;
        match entry.get_password() {
            Ok(stored) => Ok(Some(stored)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(format!("could not read the OS keyring entry: {error}")),
        }
    }

    pub(super) fn set(account: &str, value: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(super::SERVICE, account)
            .map_err(|error| format!("could not open the OS keyring entry: {error}"))?;
        entry
            .set_password(value)
            .map_err(|error| format!("could not store the file key: {error}"))
    }
}

#[cfg(test)]
mod keyring_backend {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    static MOCK: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    /// Serializes the keyring-migration tests: the mock is global like the
    /// real keyring, so tests that touch it hold this to the end.
    static SERIAL: OnceLock<Mutex<()>> = OnceLock::new();

    fn store() -> &'static Mutex<HashMap<String, String>> {
        MOCK.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub(super) fn serial_lock() -> std::sync::MutexGuard<'static, ()> {
        SERIAL
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test keyring serial lock poisoned")
    }

    pub(super) fn get(account: &str) -> Result<Option<String>, String> {
        Ok(store()
            .lock()
            .map_err(|_| "test keyring poisoned".to_string())?
            .get(account)
            .cloned())
    }

    pub(super) fn set(account: &str, value: &str) -> Result<(), String> {
        store()
            .lock()
            .map_err(|_| "test keyring poisoned".to_string())?
            .insert(account.to_string(), value.to_string());
        Ok(())
    }

    pub(super) fn clear() {
        if let Ok(mut guard) = store().lock() {
            guard.clear();
        }
    }
}

/// Read-only keyring lookup. Never creates: `None` means no entry, `Err`
/// means the backend failed (the caller warns once and tries fallback).
pub(super) fn read_keyring_key() -> Result<Option<[u8; 32]>, String> {
    match keyring_backend::get(ACCOUNT)? {
        None => Ok(None),
        Some(stored) => BASE64_STANDARD
            .decode(stored.trim())
            .map_err(|error| format!("OS keyring entry is not valid key material: {error}"))
            .and_then(|raw| {
                raw.try_into()
                    .map_err(|_| "OS keyring entry has the wrong length".to_string())
            })
            .map(Some),
    }
}

/// Store key bytes in the keyring (promotion or first write).
pub(super) fn store_keyring_key(key: &[u8; 32]) -> Result<(), String> {
    keyring_backend::set(ACCOUNT, BASE64_STANDARD.encode(key).as_str())
}

/// Generate and store a fresh keyring key. Callers must guarantee this is
/// safe: either no sealed file exists yet (first write) or the fallback key
/// already opened the files being migrated (plaintext in hand).
fn create_keyring_key() -> Result<[u8; 32], String> {
    let key = fresh_key()?;
    store_keyring_key(&key)?;
    Ok(key)
}

// ---------------------------------------------------------------------------
// Fallback key file
// ---------------------------------------------------------------------------

pub(super) fn fallback_path(root: &Path) -> PathBuf {
    root.join(FALLBACK_FILE)
}

pub(super) fn fallback_exists(root: &Path) -> bool {
    fallback_path(root).is_file()
}

/// Read-only fallback lookup. Never creates: `None` means no key file.
/// An existing file with the wrong mode/owner or bad contents fails closed.
pub(super) fn read_fallback_key(root: &Path) -> Result<Option<[u8; 32]>, String> {
    let path = fallback_path(root);
    let stored = match std::fs::read_to_string(&path) {
        Ok(stored) => stored,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };
    verify_fallback_file(&path)?;
    BASE64_STANDARD
        .decode(stored.trim())
        .map_err(|error| format!("key file {} is corrupt: {error}", path.display()))
        .and_then(|raw| {
            raw.try_into()
                .map_err(|_| format!("key file {} has the wrong length", path.display()))
        })
        .map(Some)
}

/// The fallback key file must be exactly `0600` and owned by this uid: a
/// group/world-readable fallback voids the encryption entirely.
fn verify_fallback_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = std::fs::metadata(path)
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        let mode = metadata.permissions().mode() & 0o777;
        if mode != 0o600 {
            return Err(format!(
                "refusing to use {}: mode is {mode:04o}, expected 0600",
                path.display()
            ));
        }
        // SAFETY: `getuid` takes no arguments and has no failure mode.
        let uid = unsafe { libc::getuid() };
        if metadata.uid() != uid {
            return Err(format!(
                "refusing to use {}: not owned by the current user",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Create the fallback key file atomically (`create_new`, `0600` at
/// creation — no write-then-chmod window). A racy second creator falls back
/// to reading the winner's file.
fn create_fallback_key(root: &Path) -> Result<[u8; 32], String> {
    let path = fallback_path(root);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
    }
    let key = fresh_key()?;
    let content = format!("{}\n", BASE64_STANDARD.encode(key));
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(file) => {
                use std::io::Write;
                let mut file = file;
                file.write_all(content.as_bytes())
                    .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
                file.sync_all()
                    .map_err(|error| format!("failed to sync {}: {error}", path.display()))?;
                verify_fallback_file(&path)?;
                Ok(key)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                read_fallback_key(root)?.ok_or_else(|| {
                    format!("key file {} appeared but does not open", path.display())
                })
            }
            Err(error) => Err(format!("failed to create {}: {error}", path.display())),
        }
    }
    #[cfg(not(unix))]
    {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => {
                std::fs::write(&path, content)
                    .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
                Ok(key)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                read_fallback_key(root)?.ok_or_else(|| {
                    format!("key file {} appeared but does not open", path.display())
                })
            }
            Err(error) => Err(format!("failed to create {}: {error}", path.display())),
        }
    }
}

/// Remove the fallback key file (zeroize + unlink). Returns `false` when
/// absent. Best effort on SSDs/CoW — see `super::purge_migration_backup`.
pub(super) fn remove_fallback_key(root: &Path) -> Result<bool, String> {
    let path = fallback_path(root);
    let metadata = match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!("failed to inspect {}: {error}", path.display()));
        }
        Ok(metadata) => metadata,
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "refusing to remove {}: not a regular file",
            path.display()
        ));
    }
    super::zeroize_file_contents(&path, metadata.len())?;
    std::fs::remove_file(&path)
        .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

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
pub(super) fn file_key(paths: &OrquestraPaths) -> Result<[u8; 32], String> {
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
pub(super) fn promote_fallback_to_keyring(fallback_key: &[u8; 32]) -> Result<[u8; 32], String> {
    match read_keyring_key()? {
        Some(existing) => Ok(existing),
        None => {
            store_keyring_key(fallback_key)?;
            Ok(*fallback_key)
        }
    }
}

// ---------------------------------------------------------------------------
// Migrated flag: once an install completed the encryption migration,
// plaintext secret files are refused instead of re-sealed.
// ---------------------------------------------------------------------------

/// The flag lives next to the key material, never in the data files: a
/// keyring entry when the keyring is usable, a dotfile beside `.file-key`
/// otherwise. Either one counts, so SSH→desktop transitions keep it.
pub(crate) fn is_migrated(root: &Path) -> bool {
    if keyring_usable(root) {
        match keyring_backend::get(MIGRATED_ACCOUNT) {
            Ok(Some(flag)) if flag.trim() == "1" => return true,
            _ => {}
        }
    }
    std::fs::read_to_string(root.join(MIGRATED_FILE))
        .map(|flag| flag.trim() == "1")
        .unwrap_or(false)
}

pub(super) fn mark_migrated(root: &Path) -> Result<(), String> {
    if keyring_usable(root) {
        keyring_backend::set(MIGRATED_ACCOUNT, "1")?;
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

#[cfg(test)]
/// Test hooks for the file-level migration tests in `super`: the mock
/// keyring is global like the real one, so holders serialize on the lock.
pub(crate) fn test_keyring_lock() -> std::sync::MutexGuard<'static, ()> {
    keyring_backend::serial_lock()
}

#[cfg(test)]
pub(crate) fn test_keyring_clear() {
    keyring_backend::clear();
}

#[cfg(test)]
pub(crate) fn test_keyring_raw() -> Result<Option<String>, String> {
    keyring_backend::get(ACCOUNT)
}

fn fresh_key() -> Result<[u8; 32], String> {
    use aes_gcm::aead::{KeyInit, OsRng};
    let key = aes_gcm::Aes256Gcm::generate_key(&mut OsRng);
    Ok(key.into())
}

pub(super) fn warn_once(message: String) {
    if !WARNED.swap(true, Ordering::Relaxed) {
        crate::log_warn!("{message}");
    }
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
            "orquestra-at-rest-{name}-{}-{nanos}",
            std::process::id()
        ));
        OrquestraPaths {
            root: root.clone(),
            vault: root.clone(),
            active: root,
        }
    }

    #[test]
    fn fallback_key_is_stable_per_root_and_unique_across_roots() {
        let first = test_paths("stable-a");
        let second = test_paths("stable-b");
        let key_a1 = file_key(&first).expect("key");
        let key_a2 = file_key(&first).expect("key again");
        assert_eq!(key_a1, key_a2, "cached key must be stable");
        let key_b = file_key(&second).expect("other root");
        assert_ne!(key_a1, key_b, "distinct roots get distinct keys");
        for paths in [&first, &second] {
            let key_path = paths.root.join(FALLBACK_FILE);
            assert!(key_path.is_file());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
            let _ = std::fs::remove_dir_all(&paths.root);
        }
    }

    #[test]
    fn fallback_key_promotes_into_an_empty_keyring() {
        // SSH → desktop handoff: the fallback bytes move into the keyring
        // with no new randomness; an existing keyring key is never
        // overwritten by a different fallback.
        let _serial = keyring_backend::serial_lock();
        keyring_backend::clear();
        let fallback = [0x42u8; 32];
        let promoted = promote_fallback_to_keyring(&fallback).expect("promote");
        assert_eq!(promoted, fallback);
        assert_eq!(read_keyring_key().expect("read").as_ref(), Some(&fallback));
        let other = [0x99u8; 32];
        let kept = promote_fallback_to_keyring(&other).expect("keep");
        assert_eq!(kept, fallback, "the existing keyring key wins");
        keyring_backend::clear();
    }

    #[test]
    fn read_with_sealed_file_and_no_key_creates_nothing() {
        // A read against a stranded sealed file must fail closed, not mint
        // a fresh key that could never open it (the #323 lockout).
        let paths = test_paths("no-create-on-read");
        std::fs::create_dir_all(&paths.root).expect("scratch");
        let key = file_key(&paths).expect("first-write key");
        let sealed = super::super::envelope::seal(&key, b"{}", super::super::envelope::AAD_VAULT)
            .expect("seal");
        std::fs::write(paths.root.join("vault.json"), &sealed).expect("plant sealed");
        // Drop the only key: reads must now fail without recreating it.
        std::fs::remove_file(fallback_path(&paths.root)).expect("drop key");
        cache()
            .lock()
            .expect("cache")
            .remove(&paths.root.to_string_lossy().into_owned());
        assert!(
            !fallback_exists(&paths.root),
            "precondition: no fallback key on disk"
        );
        assert!(
            read_fallback_key(&paths.root).expect("readable").is_none(),
            "no fallback key on disk"
        );
        assert!(
            file_key(&paths).is_err(),
            "even the write path must not mint a key over stranded sealed files"
        );
        assert!(
            !fallback_exists(&paths.root),
            "a failed read must not create a key file"
        );
        let _ = std::fs::remove_dir_all(&paths.root);
    }
}
