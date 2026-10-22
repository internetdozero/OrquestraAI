//! OS keyring access: reachability probe, the real backend, its hermetic
//! test mock, and the three key operations built on top of it.

use std::path::Path;

use base64::prelude::*;

use super::fresh_key;

#[cfg(not(test))]
const SERVICE: &str = "orquestra-ai";
const ACCOUNT: &str = "at-rest-file-key";
const MIGRATED_ACCOUNT: &str = "at-rest-migrated";

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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};

    static MOCK: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    static FAIL_KEY_WRITES: AtomicBool = AtomicBool::new(false);
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
        if account == super::ACCOUNT && FAIL_KEY_WRITES.load(Ordering::Relaxed) {
            return Err("injected keyring write failure".to_string());
        }
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

    pub(super) fn fail_key_writes(fail: bool) {
        FAIL_KEY_WRITES.store(fail, Ordering::Relaxed);
    }
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
    #[cfg(test)]
    if super::test_keyring_is_forced_usable() {
        return true;
    }
    !keyring_disabled(root) && keyring_reachable()
}

/// Read-only keyring lookup. Never creates: `None` means no entry, `Err`
/// means the backend failed (the caller warns once and tries fallback).
pub(crate) fn read_keyring_key() -> Result<Option<[u8; 32]>, String> {
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
pub(crate) fn store_keyring_key(key: &[u8; 32]) -> Result<(), String> {
    keyring_backend::set(ACCOUNT, BASE64_STANDARD.encode(key).as_str())
}

/// Generate and store a fresh keyring key. Callers must guarantee this is
/// safe: either no sealed file exists yet (first write) or the fallback key
/// already opened the files being migrated (plaintext in hand).
pub(crate) fn create_keyring_key() -> Result<[u8; 32], String> {
    let key = fresh_key()?;
    store_keyring_key(&key)?;
    Ok(key)
}

/// Record the completed encryption migration in the keyring.
pub(crate) fn set_migrated_flag() -> Result<(), String> {
    keyring_backend::set(MIGRATED_ACCOUNT, "1")
}

/// True when the keyring already records the completed migration.
pub(crate) fn migrated_flag() -> bool {
    matches!(keyring_backend::get(MIGRATED_ACCOUNT), Ok(Some(flag)) if flag.trim() == "1")
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

#[cfg(test)]
pub(crate) fn test_keyring_fail_key_writes(fail: bool) {
    keyring_backend::fail_key_writes(fail);
}
