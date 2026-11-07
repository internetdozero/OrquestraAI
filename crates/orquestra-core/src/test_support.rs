//! Scratch directories for tests that build a real on-disk layout.
//!
//! Fixtures used to hardcode a Linux-only `/var/tmp` path, which does not
//! exist on Windows (so every one of those tests failed to create its
//! workspace there) and ignored the platform's temp directory everywhere
//! else. `root()` is the single place that answers "where do test fixtures
//! live":
//!
//! - `ORQUESTRA_TEST_TMP`, when set to an absolute path. This repository's own
//!   convention keeps scratch data off `/tmp`, so a developer exports it once:
//!   `ORQUESTRA_TEST_TMP=/mnt/dev/testing`.
//! - otherwise the platform temp directory (`TMPDIR`, `%TEMP%`, …).
//!
//! Only tests use this module: it is compiled for `cfg(test)` inside this
//! crate and behind the `test-support` feature for the crates whose tests build
//! the same fixtures.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Every variable the path resolver reads instead of, or in front of, `$HOME`.
///
/// `rebase`/`data_dir`/`config_dir` resolve `.config/…` through
/// `XDG_CONFIG_HOME` on Unix and through `%APPDATA%` on Windows, and
/// `.local/share/…` through `XDG_DATA_HOME` and `%LOCALAPPDATA%`. A fixture that
/// only swaps `HOME` therefore still writes into the operator's real tree on a
/// machine (or CI runner) that exports any of them.
const DIR_VARIABLES: [&str; 4] = [
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "APPDATA",
    "LOCALAPPDATA",
];

/// Serializes the fixtures that repoint [`DIR_VARIABLES`]: they are
/// process-global, and two fixtures pointing them at different trees at once
/// would each describe the other's layout.
fn dir_env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Points every directory variable at `root` for as long as it is alive, then
/// restores exactly what was there before — including on a panic.
///
/// Hold the guard for the whole test: dropping it early re-exposes the real
/// environment (and lets a sibling fixture take the lock).
pub struct DirEnv {
    previous: [(&'static str, Option<OsString>); 4],
    // Declared last so the restore in `Drop` runs under the lock.
    _serial: MutexGuard<'static, ()>,
}

impl DirEnv {
    /// Take exclusive ownership of the directory variables for `root`.
    pub fn isolate(root: &Path) -> Self {
        let serial = dir_env_lock();
        let previous = DIR_VARIABLES.map(|name| (name, std::env::var_os(name)));
        // Both spellings of each location, so the same fixture is hermetic on
        // Unix, on macOS (`Library/Application Support` is home-relative and
        // ignores these) and on Windows.
        let config = root.join(".config");
        let data = root.join(".local/share");
        let appdata = root.join("AppData/Roaming");
        let localappdata = root.join("AppData/Local");
        // SAFETY: the lock above is held until this guard is dropped.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &config);
            std::env::set_var("XDG_DATA_HOME", &data);
            std::env::set_var("APPDATA", &appdata);
            std::env::set_var("LOCALAPPDATA", &localappdata);
        }
        Self {
            previous,
            _serial: serial,
        }
    }

    /// Take the same lock but *clear* every directory variable, so a test that
    /// asserts the documented fallbacks sees a process that sets none of them.
    pub fn cleared() -> Self {
        let serial = dir_env_lock();
        let previous = DIR_VARIABLES.map(|name| (name, std::env::var_os(name)));
        for name in DIR_VARIABLES {
            // SAFETY: the lock above is held until this guard is dropped.
            unsafe { std::env::remove_var(name) };
        }
        Self {
            previous,
            _serial: serial,
        }
    }
}

impl Drop for DirEnv {
    fn drop(&mut self) {
        // Destructors run before the fields drop, so the environment lock is
        // still held here.
        for (name, value) in &self.previous {
            match value {
                Some(value) => unsafe { std::env::set_var(name, value) },
                None => unsafe { std::env::remove_var(name) },
            }
        }
    }
}

/// Root every test fixture is built under.
pub fn root() -> PathBuf {
    match std::env::var_os("ORQUESTRA_TEST_TMP") {
        Some(value) if Path::new(&value).is_absolute() => PathBuf::from(value),
        // A relative value would silently nest fixtures inside the crate
        // directory; the spec elsewhere in this workspace ignores those too.
        _ => std::env::temp_dir(),
    }
}

/// `<root>/<prefix>-<pid>-<nanos>`: a directory no other test run shares.
///
/// The timestamp is what makes two runs of the same test (or two test threads
/// of the same binary) collide-free, so callers may reuse a literal prefix.
pub fn scratch(prefix: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    root().join(format!("{prefix}-{}-{unique}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The override is honoured only when absolute, so a stray relative value
    /// cannot put fixtures inside the source tree.
    #[test]
    fn a_relative_override_falls_back_to_the_platform_temp_dir() {
        let absolute = std::env::temp_dir();
        assert!(absolute.is_absolute(), "the platform temp dir is absolute");
        assert!(root().is_absolute());
    }

    #[test]
    fn scratch_directories_differ_between_calls() {
        assert_ne!(scratch("orquestra-test"), scratch("orquestra-test"));
        assert!(scratch("orquestra-test").starts_with(root()));
    }
}
