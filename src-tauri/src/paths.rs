//! Resolved storage locations for the vault, the projections and the
//! master instruction file, plus the first-run bootstrap that creates
//! them.

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{harness_ids, GLOBAL_SCOPE};

/// Optional one-time dataset used to populate an empty vault on first launch.
///
/// User-scoped by design: `<root>/seed-skills`, overridable with
/// `ORQUESTRA_SEED_SKILLS_DIR` for operators carrying an external dataset.
/// A world-writable default (the old `/mnt/amnesic` path) would let any
/// local user plant skills into someone else's first launch; whatever path
/// resolves here must additionally pass the symlink refusal in
/// [`seed_vault`].
pub(crate) fn seed_skills_dir(root: &Path) -> PathBuf {
    std::env::var_os("ORQUESTRA_SEED_SKILLS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("seed-skills"))
}

pub(crate) const AGENTS_FILE: &str = "AGENTS.md";

/// Resolved storage locations for the single-source-of-truth vault, the
/// per-scope symlink projections, and the master instruction file.
#[derive(Debug, Clone)]
pub struct OrquestraPaths {
    pub root: PathBuf,
    pub vault: PathBuf,
    pub active: PathBuf,
}

impl OrquestraPaths {
    pub fn resolve() -> Result<Self, String> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "HOME is not set; cannot locate ~/.orquestra".to_string())?;
        let root = home.join(".orquestra");

        Ok(Self {
            vault: root.join("skills-library"),
            active: root.join("active"),
            root,
        })
    }

    pub fn global_dir(&self) -> PathBuf {
        self.projection_dir(GLOBAL_SCOPE)
    }

    pub fn projection_dir(&self, scope: &str) -> PathBuf {
        self.active.join(scope)
    }

    pub fn agents_path(&self) -> PathBuf {
        self.root.join(AGENTS_FILE)
    }

    /// Create the vault, the active root, and every scope projection folder.
    /// This is idempotent and never touches existing contents.
    pub fn ensure_layout(&self) -> Result<(), String> {
        create_dir(&self.vault)?;
        create_dir(&self.active)?;
        create_dir(&self.global_dir())?;

        for harness in harness_ids() {
            create_dir(&self.projection_dir(harness))?;
        }

        Ok(())
    }

    /// Ensure the directory layout, then seed an empty vault from the
    /// user-scoped dataset on first launch.
    pub fn bootstrap(&self) -> Result<(), String> {
        self.ensure_layout()?;
        seed_vault(&self.root, &self.vault)
    }
}

pub(crate) fn create_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))
}

/// Populate an empty vault from the user-scoped seed dir, copying directory
/// trees verbatim. A missing seed directory, or a non-empty vault, is not
/// an error. A symlinked seed root fails closed: it would otherwise let an
/// attacker redirect first-launch seeding at an arbitrary tree.
fn seed_vault(root: &Path, vault: &Path) -> Result<(), String> {
    let mut entries = fs::read_dir(vault)
        .map_err(|error| format!("failed to read vault {}: {error}", vault.display()))?;
    if entries.next().is_some() {
        return Ok(());
    }

    let seed = seed_skills_dir(root);
    if fs::symlink_metadata(&seed)
        .ok()
        .is_some_and(|meta| meta.file_type().is_symlink())
    {
        return Err(format!(
            "refusing to seed from {}: not a real directory",
            seed.display()
        ));
    }
    if !seed.is_dir() {
        return Ok(());
    }

    let seed_entries = fs::read_dir(&seed)
        .map_err(|error| format!("failed to read seed dataset {}: {error}", seed.display()))?;

    for entry in seed_entries.flatten() {
        let source = entry.path();
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }

        let destination = vault.join(entry.file_name());
        if destination.exists() {
            continue;
        }

        copy_dir_all(&source, &destination)?;
    }

    Ok(())
}

pub(crate) fn copy_dir_all(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| {
        format!(
            "failed to create {} while seeding: {error}",
            destination.display()
        )
    })?;

    let entries = fs::read_dir(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?;

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("failed to read {}: {error}", source.display()))?;
        let kind = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?;
        let target = destination.join(entry.file_name());

        if kind.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target).map_err(|error| {
                format!(
                    "failed to copy {} -> {}: {error}",
                    entry.path().display(),
                    target.display()
                )
            })?;
        }
        // Symlinks and special files are intentionally skipped.
    }

    Ok(())
}

pub(crate) fn open_paths() -> Result<OrquestraPaths, String> {
    let paths = OrquestraPaths::resolve()?;
    paths.bootstrap()?;
    Ok(paths)
}

/// App-specific runtime root: `$XDG_RUNTIME_DIR/orquestra-ai` when set,
/// otherwise `<temp>/orquestra-ai-<uid>`. Never a shared world-writable
/// directory without an owner check: on multi-user machines a predictable
/// `/tmp/orquestra-ai` lets another local user pre-create it and race the
/// staging writes inside.
pub(crate) fn scratch_root() -> PathBuf {
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
pub(crate) fn ensure_scratch_root() -> Result<PathBuf, String> {
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
                    .map_err(|error| format!("failed to create {}: {error}", root.display()))?;
            }
            #[cfg(not(unix))]
            {
                std::fs::create_dir(&root)
                    .map_err(|error| format!("failed to create {}: {error}", root.display()))?;
            }
        }
        Err(error) => {
            return Err(format!("failed to inspect {}: {error}", root.display()));
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
fn verify_runtime_dir_with_uid(dir: &Path, expected_uid: u32) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let meta = std::fs::symlink_metadata(dir)
        .map_err(|error| format!("failed to inspect {}: {error}", dir.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "refusing to stage in {}: not a real directory",
            dir.display()
        ));
    }
    if !meta.is_dir() {
        return Err(format!(
            "refusing to stage in {}: not a directory",
            dir.display()
        ));
    }
    if meta.uid() != expected_uid {
        return Err(format!(
            "refusing to stage in {}: not owned by the current user",
            dir.display()
        ));
    }
    if meta.permissions().mode() & 0o777 != 0o700 {
        return Err(format!(
            "refusing to stage in {}: must be mode 0700",
            dir.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn verify_runtime_dir(dir: &Path) -> Result<(), String> {
    // SAFETY: `getuid` takes no arguments and has no failure mode.
    let uid = unsafe { libc::getuid() };
    verify_runtime_dir_with_uid(dir, uid)
}

#[cfg(not(unix))]
fn verify_runtime_dir(dir: &Path) -> Result<(), String> {
    if !dir.is_dir() {
        return Err(format!(
            "refusing to stage in {}: not a directory",
            dir.display()
        ));
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
pub(crate) fn unique_tmp_suffix() -> String {
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
pub(crate) fn fresh_scratch_dir(prefix: &str) -> Result<PathBuf, String> {
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
            .map_err(|error| format!("failed to create {}: {error}", dir.display()))?;
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(&dir)
            .map_err(|error| format!("failed to create {}: {error}", dir.display()))?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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

    fn temp_seed_root(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "orquestra-seed-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("vault")).expect("vault dir");
        root
    }

    /// Task #281: the default seed lives inside the storage root, never on
    /// a world-writable mount.
    #[test]
    fn seed_defaults_to_a_dir_inside_the_storage_root() {
        // The operator override is process-global; only assert the default
        // when the test runner did not set it.
        if std::env::var_os("ORQUESTRA_SEED_SKILLS_DIR").is_none() {
            let root = PathBuf::from("/tmp/orquestra-test-root");
            assert_eq!(seed_skills_dir(&root), root.join("seed-skills"));
        }
    }

    /// Task #281: first launch copies the user-scoped seed into an empty vault.
    #[test]
    fn seed_vault_copies_from_the_user_scoped_seed_dir() {
        let root = temp_seed_root("copy");
        let vault = root.join("vault");
        fs::create_dir_all(root.join("seed-skills/demo")).expect("seed skill");
        fs::write(root.join("seed-skills/demo/SKILL.md"), "demo\n").expect("seed file");
        seed_vault(&root, &vault).expect("seed");
        assert!(vault.join("demo/SKILL.md").is_file());
        let _ = fs::remove_dir_all(&root);
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
        let previous = std::env::var_os("XDG_RUNTIME_DIR");
        std::env::set_var("XDG_RUNTIME_DIR", &xdg);
        let root = scratch_root();
        match previous {
            Some(value) => std::env::set_var("XDG_RUNTIME_DIR", value),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
        assert_eq!(root, xdg.join("orquestra-ai"));
        let _ = fs::remove_dir_all(&xdg);
    }

    /// Task #325: without `XDG_RUNTIME_DIR` the fallback carries the uid.
    #[cfg(unix)]
    #[test]
    fn scratch_root_fallback_is_per_uid() {
        let previous = std::env::var_os("XDG_RUNTIME_DIR");
        std::env::remove_var("XDG_RUNTIME_DIR");
        let root = scratch_root();
        match previous {
            Some(value) => std::env::set_var("XDG_RUNTIME_DIR", value),
            None => {}
        }
        // SAFETY: `getuid` takes no arguments and has no failure mode.
        let uid = unsafe { libc::getuid() };
        assert_eq!(
            root,
            std::env::temp_dir().join(format!("orquestra-ai-{uid}"))
        );
    }

    /// Task #325: `ensure_scratch_root` creates the runtime dir `0700`.
    #[test]
    fn ensure_scratch_root_creates_0700() {
        let root = ensure_scratch_root().expect("runtime root");
        assert_eq!(root, scratch_root());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&root).unwrap().permissions().mode() & 0o777,
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
        let base = temp_seed_root("runtime-refuse");
        // Symlink fails closed.
        let real = base.join("real");
        fs::create_dir_all(&real).expect("real dir");
        std::os::unix::fs::symlink(&real, base.join("linked")).expect("link");
        assert!(
            verify_runtime_dir_with_uid(&base.join("linked"), unsafe { libc::getuid() }).is_err(),
            "a symlinked runtime dir must be refused"
        );
        // A directory owned by someone else fails closed.
        let owned = base.join("owned");
        fs::create_dir_all(&owned).expect("owned dir");
        fs::set_permissions(&owned, fs::Permissions::from_mode(0o700)).expect("chmod");
        // SAFETY: `getuid` takes no arguments and has no failure mode.
        let uid = unsafe { libc::getuid() };
        assert!(
            verify_runtime_dir_with_uid(&owned, uid.wrapping_add(1)).is_err(),
            "a foreign-owned runtime dir must be refused"
        );
        assert!(verify_runtime_dir_with_uid(&owned, uid).is_ok());
        // A group/world-accessible directory fails closed.
        fs::set_permissions(&owned, fs::Permissions::from_mode(0o755)).expect("loosen");
        assert!(
            verify_runtime_dir_with_uid(&owned, uid).is_err(),
            "a 0755 runtime dir must be refused"
        );
        // A regular file is not a directory.
        let file = base.join("file");
        fs::write(&file, b"x").expect("file");
        assert!(verify_runtime_dir_with_uid(&file, uid).is_err());
        let _ = fs::remove_dir_all(&base);
    }

    /// Task #281: a symlinked seed root fails closed instead of seeding the
    /// vault from an attacker-chosen tree.
    #[cfg(unix)]
    #[test]
    fn seed_vault_refuses_a_symlinked_seed_dir() {
        let root = temp_seed_root("link");
        let vault = root.join("vault");
        let outside = root.join("outside");
        fs::create_dir_all(outside.join("evil")).expect("outside");
        fs::write(outside.join("evil/SKILL.md"), "evil\n").expect("evil file");
        std::os::unix::fs::symlink(&outside, root.join("seed-skills")).expect("link");
        assert!(
            seed_vault(&root, &vault).is_err(),
            "a symlinked seed must not plant skills"
        );
        assert!(
            fs::read_dir(&vault).expect("read vault").next().is_none(),
            "the vault must stay empty"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
