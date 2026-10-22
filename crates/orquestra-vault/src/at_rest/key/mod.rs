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
pub(crate) use resolve::{file_key, mark_migrated, promote_fallback_to_keyring};
pub(crate) use ring::{keyring_usable, read_keyring_key};
#[cfg(test)]
pub(crate) use ring::{test_keyring_clear, test_keyring_lock, test_keyring_raw};

static CACHE: OnceLock<Mutex<HashMap<String, [u8; 32]>>> = OnceLock::new();
static WARNED: AtomicBool = AtomicBool::new(false);

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
