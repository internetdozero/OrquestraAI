//! File-key resolution: OS keyring first, 0600 file fallback.
//!
//! One random 256-bit key encrypts both secret files (`vault.json`,
//! `mcps.json`). The key lives in the OS keyring (Secret Service on Linux,
//! Keychain on macOS, Credential Manager on Windows). When no keyring is
//! reachable — headless hosts, CI, locked collections — the backend falls
//! back to a `0600` key file beside the secrets and warns loudly once per
//! process, so the degraded mode is explicit rather than silent.
//!
//! Resolved keys are cached in memory per storage root, so the keyring (or
//! the fallback file) is touched once per process, not per request.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, OnceLock,
};

use base64::prelude::*;

use crate::paths::OrquestraPaths;

const SERVICE: &str = "orquestra-ai";
const ACCOUNT: &str = "at-rest-file-key";
const FALLBACK_FILE: &str = ".file-key";

static CACHE: OnceLock<Mutex<HashMap<String, [u8; 32]>>> = OnceLock::new();
static WARNED: AtomicBool = AtomicBool::new(false);

fn cache() -> &'static Mutex<HashMap<String, [u8; 32]>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The file key for one storage root, cached in memory after first use.
pub(super) fn file_key(paths: &OrquestraPaths) -> Result<[u8; 32], String> {
    let scope = paths.root.to_string_lossy().into_owned();
    if let Some(hit) = cache()
        .lock()
        .map_err(|_| "file-key cache poisoned".to_string())?
        .get(&scope)
    {
        return Ok(*hit);
    }
    let key = resolve(&paths.root)?;
    cache()
        .lock()
        .map_err(|_| "file-key cache poisoned".to_string())?
        .insert(scope, key);
    Ok(key)
}

fn resolve(root: &Path) -> Result<[u8; 32], String> {
    // Unit tests must never touch the operator's real keyring: every test
    // writes temp-dir vaults, so the 0600 fallback key is the hermetic path.
    if cfg!(test) {
        return fallback_key(root);
    }
    if has_session_bus() {
        match keyring_key() {
            Ok(key) => return Ok(key),
            Err(error) => warn_once(format!(
                "OS keyring unreachable ({error}); using a 0600 key file — \
                 theft-of-disk protection is degraded until a keyring is available"
            )),
        }
    }
    fallback_key(root)
}

/// Native backends need no D-Bus. On Linux, skip the keyring fast when no
/// session bus exists instead of waiting out a secret-service timeout.
fn has_session_bus() -> bool {
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

fn keyring_key() -> Result<[u8; 32], String> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|error| format!("could not open the OS keyring entry: {error}"))?;
    match entry.get_password() {
        Ok(stored) => BASE64_STANDARD
            .decode(stored.trim())
            .map_err(|error| format!("OS keyring entry is not valid key material: {error}"))
            .and_then(|raw| {
                raw.try_into()
                    .map_err(|_| "OS keyring entry has the wrong length".to_string())
            }),
        Err(keyring::Error::NoEntry) => {
            let key = fresh_key()?;
            entry
                .set_password(BASE64_STANDARD.encode(key).as_str())
                .map_err(|error| format!("could not store the file key: {error}"))?;
            Ok(key)
        }
        Err(error) => Err(format!("could not read the OS keyring entry: {error}")),
    }
}

/// 0600 key file beside the secrets. A chmod miss fails closed: a
/// world-readable fallback key would void the encryption entirely.
fn fallback_key(root: &Path) -> Result<[u8; 32], String> {
    let path = root.join(FALLBACK_FILE);
    if let Ok(stored) = std::fs::read_to_string(&path) {
        return BASE64_STANDARD
            .decode(stored.trim())
            .map_err(|error| format!("key file {} is corrupt: {error}", path.display()))
            .and_then(|raw| {
                raw.try_into()
                    .map_err(|_| format!("key file {} has the wrong length", path.display()))
            });
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
    }
    let key = fresh_key()?;
    std::fs::write(&path, format!("{}\n", BASE64_STANDARD.encode(key)))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to lock down {}: {error}", path.display()))?;
    }
    Ok(key)
}

fn fresh_key() -> Result<[u8; 32], String> {
    use aes_gcm::aead::{KeyInit, OsRng};
    let key = aes_gcm::Aes256Gcm::generate_key(&mut OsRng);
    Ok(key.into())
}

fn warn_once(message: String) {
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
        // Fresh process view: drop the cache entry by using another root.
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
}
