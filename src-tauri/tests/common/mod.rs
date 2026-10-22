//! Shared isolation for integration tests: every suite mutates `HOME` to
//! point at a scratch tree, and `HOME` is process-global, so all tests in
//! one binary serialize on a mutex and restore the original value after.
//! Each `tests/*.rs` file is its own binary (and process), so one lock per
//! consumer via this module is enough.

// Every `tests/*.rs` binary compiles this module on its own, so a helper only
// some binaries call is dead code in the others. Sharing the fixture is worth
// more than pruning it per binary.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static HOME_LOCK: Mutex<()> = Mutex::new(());

/// Hermetic keyring isolation: scratch suites must never read (or create) the
/// operator's real OS keyring entry, which is global per service/account and
/// shared with the host install.
const KEYRING_FLAG: &str = "ORQUESTRA_KEYRING_DISABLE";

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "orquestra-it-{label}-{}-{nanos}-{seq}",
        std::process::id()
    ))
}

/// Run `body` with `HOME` pointed at a fresh scratch tree. The previous
/// value is restored afterwards and the tree removed, so the real
/// `~/.orquestra` is never touched and parallel test binaries cannot share
/// state through colliding paths.
pub fn with_temp_home<T>(label: &str, body: impl FnOnce(&Path) -> T) -> T {
    with_scratch_home(label, false, body)
}

/// Run `body` the way a Windows launch presents the environment: `HOME`
/// removed, `USERPROFILE` pointing at the fresh scratch tree.
///
/// This is the shape the app has to survive off Unix, so it is the only place
/// the fallback in `orquestra_core::paths::home_dir` is exercised end to end.
pub fn with_userprofile_only<T>(label: &str, body: impl FnOnce(&Path) -> T) -> T {
    with_scratch_home(label, true, body)
}

/// The shared scaffolding: one lock, one scratch tree, every touched variable
/// captured and restored.
///
/// `windows_style` removes `HOME` and sets `USERPROFILE` instead. The Unix
/// shape deliberately leaves `USERPROFILE` alone, so a broken `HOME` lookup
/// fails the test rather than silently resolving through the fallback.
fn with_scratch_home<T>(label: &str, windows_style: bool, body: impl FnOnce(&Path) -> T) -> T {
    let _guard = HOME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous_home = std::env::var_os("HOME");
    let previous_profile = std::env::var_os("USERPROFILE");
    let previous_flag = std::env::var_os(KEYRING_FLAG);

    let home = unique_dir(label);
    std::fs::create_dir_all(&home).expect("scratch home");

    unsafe {
        if windows_style {
            std::env::remove_var("HOME");
            std::env::set_var("USERPROFILE", &home);
        } else {
            std::env::set_var("HOME", &home);
        }
        std::env::set_var(KEYRING_FLAG, "1");
    }

    let outcome = body(&home);

    restore("HOME", previous_home);
    restore("USERPROFILE", previous_profile);
    restore(KEYRING_FLAG, previous_flag);
    let _ = std::fs::remove_dir_all(&home);
    outcome
}

/// Put one variable back exactly as it was: a value restores it, absence
/// removes it.
fn restore(name: &'static str, previous: Option<std::ffi::OsString>) {
    match previous {
        Some(value) => unsafe { std::env::set_var(name, value) },
        None => unsafe { std::env::remove_var(name) },
    }
}
