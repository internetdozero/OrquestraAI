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
//!
//! The degraded mode is also observable: [`key_status`] reports which backend
//! protects a root and, for the file fallback, why — a warning in the log is
//! invisible to an operator in the desktop app, who otherwise has no way to
//! learn that theft-of-disk protection dropped.
//!
//! Split by job: [`ring`] owns the OS keyring (plus its hermetic test mock),
//! [`fallback`] the `0600` key file, and [`resolve`] the decision of which
//! key seals or opens a given root, including the migrated flag.

mod fallback;
mod resolve;
mod ring;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, OnceLock,
};

#[cfg(test)]
pub(crate) use fallback::fallback_path;
pub(crate) use fallback::{fallback_exists, read_fallback_key, remove_fallback_key};
pub(crate) use resolve::is_migrated;
pub use resolve::key_status;
pub(crate) use resolve::{file_key, mark_migrated, promote_fallback_to_keyring};
pub(crate) use ring::{keyring_usable, read_keyring_key};
#[cfg(test)]
pub(crate) use ring::{
    test_keyring_clear, test_keyring_fail_key_writes, test_keyring_lock, test_keyring_raw,
};

/// Which backend holds the key that protects a root's secret files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyBackend {
    /// OS keyring: Secret Service, macOS Keychain, Windows Credential Manager.
    Keyring,
    /// A `0600` key file beside the secrets: no protection against disk theft.
    FileKey,
}

impl KeyBackend {
    /// Stable identifier for the IPC layer and the UI.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keyring => "keyring",
            Self::FileKey => "file-key",
        }
    }

    /// `true` when the key sits next to the ciphertext it protects.
    pub fn is_degraded(self) -> bool {
        self == Self::FileKey
    }
}

/// Backend in use for one storage root, with the reason when it degraded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyStatus {
    pub backend: KeyBackend,
    /// Why the file fallback is in use; `None` for the keyring.
    pub reason: Option<String>,
}

impl KeyStatus {
    pub(crate) fn keyring() -> Self {
        Self {
            backend: KeyBackend::Keyring,
            reason: None,
        }
    }

    pub(crate) fn file_key(reason: impl Into<String>) -> Self {
        Self {
            backend: KeyBackend::FileKey,
            reason: Some(reason.into()),
        }
    }

    /// `true` when theft-of-disk protection is degraded.
    pub fn is_degraded(&self) -> bool {
        self.backend.is_degraded()
    }
}

static CACHE: OnceLock<Mutex<HashMap<String, [u8; 32]>>> = OnceLock::new();
/// Last backend decided per root. Keyed like [`CACHE`]: one process serves
/// several roots (tests, imports), so a single global answer would lie.
static OBSERVED: OnceLock<Mutex<HashMap<String, KeyStatus>>> = OnceLock::new();
static WARNED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
thread_local! {
    static TEST_FORCE_KEYRING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) struct ForcedKeyringUsable;

#[cfg(test)]
pub(crate) fn force_keyring_usable_for_test() -> ForcedKeyringUsable {
    TEST_FORCE_KEYRING.with(|flag| flag.set(true));
    ForcedKeyringUsable
}

#[cfg(test)]
impl Drop for ForcedKeyringUsable {
    fn drop(&mut self) {
        TEST_FORCE_KEYRING.with(|flag| flag.set(false));
    }
}

#[cfg(test)]
fn test_keyring_is_forced_usable() -> bool {
    TEST_FORCE_KEYRING.with(std::cell::Cell::get)
}

fn cache() -> &'static Mutex<HashMap<String, [u8; 32]>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn cache_insert(root: &Path, key: [u8; 32]) {
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

fn observed() -> &'static Mutex<HashMap<String, KeyStatus>> {
    OBSERVED.get_or_init(|| Mutex::new(HashMap::new()))
}

fn observe(root: &Path, status: KeyStatus) {
    if let Ok(mut guard) = observed().lock() {
        guard.insert(root.to_string_lossy().into_owned(), status);
    }
}

/// Last backend this process resolved for `root`. The most recent decision
/// wins: a promotion into the keyring must replace an earlier fallback answer.
pub(crate) fn observed_status(root: &Path) -> Option<KeyStatus> {
    observed()
        .lock()
        .ok()
        .and_then(|guard| guard.get(&root.to_string_lossy().into_owned()).cloned())
}

pub(crate) fn observe_keyring(root: &Path) {
    observe(root, KeyStatus::keyring());
}

pub(crate) fn observe_file_key(root: &Path, reason: &str) {
    observe(root, KeyStatus::file_key(reason));
}

#[cfg(test)]
pub(crate) fn forget_observed(root: &Path) {
    if let Ok(mut guard) = observed().lock() {
        guard.remove(&root.to_string_lossy().into_owned());
    }
}

fn fresh_key() -> Result<[u8; 32], String> {
    use aes_gcm::aead::{KeyInit, OsRng};
    let key = aes_gcm::Aes256Gcm::generate_key(&mut OsRng);
    Ok(key.into())
}

pub(crate) fn warn_once(message: String) {
    if !WARNED.swap(true, Ordering::Relaxed) {
        crate::log_warn!("{message}");
    }
}
