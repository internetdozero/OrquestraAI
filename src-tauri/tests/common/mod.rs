//! Shared isolation for integration tests: every suite mutates `HOME` to
//! point at a scratch tree, and `HOME` is process-global, so all tests in
//! one binary serialize on a mutex and restore the original value after.
//! Each `tests/*.rs` file is its own binary (and process), so one lock per
//! consumer via this module is enough.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static HOME_LOCK: Mutex<()> = Mutex::new(());
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
    let _guard = HOME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = std::env::var_os("HOME");
    // Hermetic keyring isolation: scratch suites must never read (or create)
    // the operator's real OS keyring entry, which is global per
    // service/account and shared with the host install. Belt and suspenders
    // with the temp-root check in `at_rest::key`: the flag covers relocated
    // temp dirs, the root check covers callers that bypass this helper.
    let previous_flag = std::env::var_os("ORQUESTRA_KEYRING_DISABLE");
    let home = unique_dir(label);
    std::fs::create_dir_all(&home).expect("scratch HOME");
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("ORQUESTRA_KEYRING_DISABLE", "1");
    }
    let outcome = body(&home);
    match previous {
        Some(value) => unsafe {
            std::env::set_var("HOME", value);
        },
        None => unsafe {
            std::env::remove_var("HOME");
        },
    }
    match previous_flag {
        Some(value) => unsafe {
            std::env::set_var("ORQUESTRA_KEYRING_DISABLE", value);
        },
        None => unsafe {
            std::env::remove_var("ORQUESTRA_KEYRING_DISABLE");
        },
    }
    let _ = std::fs::remove_dir_all(&home);
    outcome
}
