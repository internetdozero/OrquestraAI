//! Per-process runtime and staging storage: the XDG runtime root, its
//! owner/mode guard, unique temp-file suffixes and staging directories.

use std::path::{Path, PathBuf};

use crate::error::{CoreError, CoreResult};

/// App-specific runtime root: `$XDG_RUNTIME_DIR/orquestra-ai` when set,
/// otherwise `<temp>/orquestra-ai-<uid>`. Never a shared world-writable
/// directory without an owner check: on multi-user machines a predictable
/// `/tmp/orquestra-ai` lets another local user pre-create it and race the
/// staging writes inside.
pub fn scratch_root() -> PathBuf {
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        if !runtime.is_empty() {
            return PathBuf::from(runtime).join("orquestra-ai");
        }
    }
    #[cfg(unix)]
    {
        // SAFETY: `getuid` takes no arguments and has no failure mode.
        let uid = unsafe { libc::getuid() };
        return std::env::temp_dir().join(format!("orquestra-ai-{uid}"));
    }
    #[cfg(not(unix))]
    {
        return std::env::temp_dir().join("orquestra-ai");
    }
}

/// Create [`scratch_root`] when missing and verify it on every call: it must
/// be a real directory (never a symlink), owned by this uid, with mode
/// `0700` (Unix). Anything else fails closed instead of staging downloads
/// into attacker-visible or attacker-writable storage.
pub fn ensure_scratch_root() -> CoreResult<PathBuf> {
    let root = scratch_root();
    match std::fs::symlink_metadata(&root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // One level only: every parent (`$XDG_RUNTIME_DIR`, the temp dir)
            // must already exist, so a missing parent fails instead of
            // building an unexpected tree.
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                std::fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&root)
                    .map_err(|error| CoreError::io("create", &root, error))?;
            }
            #[cfg(not(unix))]
            {
                std::fs::create_dir(&root)
                    .map_err(|error| CoreError::io("create", &root, error))?;
            }
        }
        Err(error) => {
            return Err(CoreError::io("inspect", &root, error));
        }
        Ok(_) => {}
    }
    verify_runtime_dir(&root)?;
    Ok(root)
}

/// `lstat` the runtime root: no symlinks, real directory, ours, `0700`.
/// `expected_uid` is injected so tests can prove the ownership refusal
/// without needing a second Unix user.
#[cfg(unix)]
fn verify_runtime_dir_with_uid(dir: &Path, expected_uid: u32) -> CoreResult<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let meta =
        std::fs::symlink_metadata(dir).map_err(|error| CoreError::io("inspect", dir, error))?;
    if meta.file_type().is_symlink() {
        return Err(CoreError::unsafe_path(
            "stage in",
            dir,
            "not a real directory",
        ));
    }
    if !meta.is_dir() {
        return Err(CoreError::unsafe_path("stage in", dir, "not a directory"));
    }
    if meta.uid() != expected_uid {
        return Err(CoreError::unsafe_path(
            "stage in",
            dir,
            "not owned by the current user",
        ));
    }
    if meta.permissions().mode() & 0o777 != 0o700 {
        return Err(CoreError::unsafe_path("stage in", dir, "must be mode 0700"));
    }
    Ok(())
}

#[cfg(unix)]
fn verify_runtime_dir(dir: &Path) -> CoreResult<()> {
    // SAFETY: `getuid` takes no arguments and has no failure mode.
    let uid = unsafe { libc::getuid() };
    verify_runtime_dir_with_uid(dir, uid)
}

#[cfg(not(unix))]
fn verify_runtime_dir(dir: &Path) -> CoreResult<()> {
    if !dir.is_dir() {
        return Err(CoreError::unsafe_path("stage in", dir, "not a directory"));
    }
    Ok(())
}

/// Unique suffix for atomic-write temp files (`.{name}.tmp-{suffix}` or
/// `{stem}.tmp-{suffix}`).
///
/// PID alone collides across Tauri IPC threads in one process, so two
/// concurrent saves would truncate and rename the same temp file and
/// interleave their bytes. Thread id + nanos + a process-wide counter make
/// every attempt unique. Temp files stay beside their target (same
/// filesystem), so the final rename is still atomic.
pub fn unique_tmp_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let thread_tag: String = format!("{:?}", std::thread::current().id())
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("tmp-{}-{thread_tag}-{nanos}-{seq}", std::process::id())
}

/// Create a fresh unique scratch dir under [`scratch_root`] with `0o700`
/// permissions on Unix. The caller owns cleanup (`remove_dir_all`).
/// Uniqueness comes from pid + nanos + an atomic counter, so concurrent
/// imports/updates never share a staging tree.
pub fn fresh_scratch_dir(prefix: &str) -> CoreResult<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let root = ensure_scratch_root()?;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = root.join(format!("{prefix}-{}-{unique}-{seq}", std::process::id()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|error| CoreError::io("create", &dir, error))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir(&dir).map_err(|error| CoreError::io("create", &dir, error))?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::ffi::{OsStr, OsString};
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// Serializes the tests that read or rewrite `XDG_RUNTIME_DIR`.
    ///
    /// The environment is process-global, so one test removing the variable
    /// mid-flight would make [`scratch_root`] return — and
    /// [`ensure_scratch_root`] create — a path another test set up. The lock
    /// makes each test's view of the variable stable for its whole scope.
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Holds the env lock and pins `XDG_RUNTIME_DIR` to `value` for the
    /// guarded scope, restoring the previous value on drop (panic included).
    struct RuntimeDir {
        previous: Option<OsString>,
        _serial: MutexGuard<'static, ()>,
    }

    impl RuntimeDir {
        fn set(value: Option<&OsStr>) -> Self {
            let serial = env_lock();
            let previous = std::env::var_os("XDG_RUNTIME_DIR");
            match value {
                Some(value) => std::env::set_var("XDG_RUNTIME_DIR", value),
                None => std::env::remove_var("XDG_RUNTIME_DIR"),
            }
            Self {
                previous,
                _serial: serial,
            }
        }
    }

    impl Drop for RuntimeDir {
        fn drop(&mut self) {
            // Runs before the fields drop, so the restore happens while the
            // env lock is still held.
            match self.previous.take() {
                Some(value) => std::env::set_var("XDG_RUNTIME_DIR", value),
                None => std::env::remove_var("XDG_RUNTIME_DIR"),
            }
        }
    }

    /// The regression this helper exists for: concurrent temp files in one
    /// process must never share a name, or two IPC threads truncate and
    /// rename each other's payload mid-write.
    #[test]
    fn tmp_suffixes_are_unique_across_threads() {
        let seen = std::sync::Mutex::new(HashSet::new());
        std::thread::scope(|scope| {
            for _ in 0..16 {
                scope.spawn(|| {
                    for _ in 0..500 {
                        let mut guard = seen.lock().unwrap_or_else(|p| p.into_inner());
                        assert!(guard.insert(unique_tmp_suffix()), "tmp name reused");
                    }
                });
            }
        });
        assert_eq!(seen.lock().unwrap_or_else(|p| p.into_inner()).len(), 8000);
    }

    fn temp_root(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "orquestra-seed-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("vault")).expect("vault dir");
        root
    }

    /// Task #325: `$XDG_RUNTIME_DIR` wins when set; otherwise the fallback
    /// is per-uid (`orquestra-ai-<uid>`), never a shared predictable path.
    #[test]
    fn scratch_root_prefers_xdg_runtime_dir() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let xdg =
            std::env::temp_dir().join(format!("orquestra-xdg-{}-{nanos}", std::process::id()));
        let _env = RuntimeDir::set(Some(xdg.as_os_str()));
        let root = scratch_root();
        assert_eq!(root, xdg.join("orquestra-ai"));
        let _ = std::fs::remove_dir_all(&xdg);
    }

    /// Task #325: without `XDG_RUNTIME_DIR` the fallback carries the uid.
    #[cfg(unix)]
    #[test]
    fn scratch_root_fallback_is_per_uid() {
        let _env = RuntimeDir::set(None);
        let root = scratch_root();
        // SAFETY: `getuid` takes no arguments and has no failure mode.
        let uid = unsafe { libc::getuid() };
        assert_eq!(
            root,
            std::env::temp_dir().join(format!("orquestra-ai-{uid}"))
        );
    }

    /// Task #325: `ensure_scratch_root` creates the runtime dir `0700`.
    ///
    /// Both branches run under an explicit `XDG_RUNTIME_DIR`, never the one
    /// the machine happens to export: the assertion is about this function's
    /// permissions, not about the host.
    #[test]
    fn ensure_scratch_root_creates_0700() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let xdg = std::env::temp_dir().join(format!(
            "orquestra-xdg-create-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&xdg).expect("xdg dir");
        {
            let _env = RuntimeDir::set(Some(xdg.as_os_str()));
            let root = ensure_scratch_root().expect("runtime root");
            assert_eq!(root, xdg.join("orquestra-ai"));
            assert!(
                root.is_dir(),
                "the runtime dir is created when it is missing"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                    0o700
                );
            }
        }
        let _ = std::fs::remove_dir_all(&xdg);

        // The per-uid fallback path creates and verifies the same way.
        let _env = RuntimeDir::set(None);
        let root = ensure_scratch_root().expect("fallback runtime root");
        assert_eq!(root, scratch_root());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
    }

    /// Task #325: a symlinked or foreign runtime dir is refused (Unix).
    /// Ownership is proven by injecting a wrong uid — no second user needed.
    #[cfg(unix)]
    #[test]
    fn runtime_dir_refuses_symlinks_foreign_owners_and_loose_modes() {
        use std::os::unix::fs::PermissionsExt;
        let base = temp_root("runtime-refuse");
        // Symlink fails closed.
        let real = base.join("real");
        std::fs::create_dir_all(&real).expect("real dir");
        std::os::unix::fs::symlink(&real, base.join("linked")).expect("link");
        assert!(
            verify_runtime_dir_with_uid(&base.join("linked"), unsafe { libc::getuid() }).is_err(),
            "a symlinked runtime dir must be refused"
        );
        // A directory owned by someone else fails closed.
        let owned = base.join("owned");
        std::fs::create_dir_all(&owned).expect("owned dir");
        std::fs::set_permissions(&owned, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        // SAFETY: `getuid` takes no arguments and has no failure mode.
        let uid = unsafe { libc::getuid() };
        assert!(
            verify_runtime_dir_with_uid(&owned, uid.wrapping_add(1)).is_err(),
            "a foreign-owned runtime dir must be refused"
        );
        assert!(verify_runtime_dir_with_uid(&owned, uid).is_ok());
        // A group/world-accessible directory fails closed.
        std::fs::set_permissions(&owned, std::fs::Permissions::from_mode(0o755)).expect("loosen");
        assert!(
            verify_runtime_dir_with_uid(&owned, uid).is_err(),
            "a 0755 runtime dir must be refused"
        );
        // A regular file is not a directory.
        let file = base.join("file");
        std::fs::write(&file, b"x").expect("file");
        assert!(verify_runtime_dir_with_uid(&file, uid).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}
