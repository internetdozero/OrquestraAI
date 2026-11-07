//! Process-global environment isolation for the crate's unit tests.
//!
//! Unit tests share one process, so a test that reads `~/.orquestra` sees
//! whatever the machine's real `HOME` holds — and a sibling test that briefly
//! repoints `XDG_DATA_HOME` can make that read resolve a different at-rest key
//! mid-flight, which surfaces as `wrong key or corrupted file` on a fresh CI
//! runner. Integration tests already isolate `HOME` per binary
//! (`tests/common/mod.rs`); this is the same idea for the lib target.
//!
//! [`isolated_home`] points `HOME` at one scratch tree for the whole binary and
//! returns a guard: the caller holds it for the duration of the test, which
//! serializes every store-reading test instead of letting them interleave. The
//! value is written once, so even a reader that somehow skipped the lock sees a
//! consistent tree.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

static HOME_LOCK: Mutex<()> = Mutex::new(());
static SCRATCH_HOME: OnceLock<PathBuf> = OnceLock::new();

/// Point `HOME` at a scratch tree (on first use) and lock out the sibling
/// tests that touch the same store.
///
/// Panics when it cannot create the tree: a test that silently kept the real
/// `HOME` would be back to reading the operator's store.
pub(crate) fn isolated_home() -> MutexGuard<'static, ()> {
    let guard = HOME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    SCRATCH_HOME.get_or_init(|| {
        let home = orquestra_core::test_support::scratch("lib-home");
        std::fs::create_dir_all(&home).expect("create the scratch HOME for unit tests");
        // The store lives under `HOME`, but the at-rest key and the harness
        // layouts resolve through the platform's directory variables too, so
        // point those at the same tree instead of the machine's.
        let config = home.join(".config");
        let data = home.join(".local/share");
        let appdata = home.join("AppData/Roaming");
        let localappdata = home.join("AppData/Local");
        // SAFETY: this runs inside the guard, before any test of this binary
        // reads these variables, and the values are never rewritten afterwards.
        unsafe {
            std::env::set_var("HOME", &home);
            std::env::set_var("XDG_CONFIG_HOME", &config);
            std::env::set_var("XDG_DATA_HOME", &data);
            std::env::set_var("APPDATA", &appdata);
            std::env::set_var("LOCALAPPDATA", &localappdata);
            // The OS keyring is global per account, so a scratch suite must
            // never read or create the operator's real entry.
            std::env::set_var("ORQUESTRA_KEYRING_DISABLE", "1");
        }
        home
    });

    guard
}
