//! Test-only guard for the process environment.
//!
//! Cargo runs one crate's tests on several threads, and `set_var`/`remove_var`
//! change state every thread can observe. Without coordination a test can read
//! a variable that a sibling temporarily removed — the flake that made
//! `ensure_scratch_root_creates_0700` see another test's `XDG_RUNTIME_DIR`.
//!
//! One mutex shared by every guarded test, plus a restore-on-drop guard, keeps
//! each test's view of the environment stable for its whole scope, including
//! when an assertion panics.

use std::ffi::{OsStr, OsString};
use std::sync::{Mutex, MutexGuard, OnceLock};

fn lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Holds the environment lock and restores `name` to its original value when
/// dropped.
///
/// Capture one with [`EnvGuard::capture`], then [`EnvGuard::set`] the variable
/// as often as the test needs: every mutation stays inside the same lock, and
/// the previous value comes back even if the test panics.
pub(crate) struct EnvGuard {
    name: &'static str,
    previous: Option<OsString>,
    _serial: MutexGuard<'static, ()>,
}

impl EnvGuard {
    /// Takes the lock and remembers `name`'s current value.
    pub(crate) fn capture(name: &'static str) -> Self {
        let serial = lock();
        let previous = std::env::var_os(name);
        Self {
            name,
            previous,
            _serial: serial,
        }
    }

    /// Sets `name` while the guard holds the lock, removing it for `None`.
    pub(crate) fn set(&self, value: Option<&OsStr>) {
        match value {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // Struct destructors run before the fields drop, so the restore
        // happens while the environment lock is still held.
        match self.previous.take() {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
}
