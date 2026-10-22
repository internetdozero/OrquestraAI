//! Resolved storage locations for the vault, the projections and the
//! master instruction file, plus the first-run bootstrap that creates
//! them.

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{harness_ids, GLOBAL_SCOPE};

/// Optional one-time dataset used to populate an empty vault on first launch.
const SEED_DIR: &str = "/mnt/amnesic/temporary_skills";

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
    /// temporary dataset on first launch.
    pub fn bootstrap(&self) -> Result<(), String> {
        self.ensure_layout()?;
        seed_vault(&self.vault)
    }
}

pub(crate) fn create_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))
}

/// Populate an empty vault from `SEED_DIR`, copying directory trees verbatim.
/// A missing seed directory, or a non-empty vault, is not an error.
fn seed_vault(vault: &Path) -> Result<(), String> {
    let mut entries = fs::read_dir(vault)
        .map_err(|error| format!("failed to read vault {}: {error}", vault.display()))?;
    if entries.next().is_some() {
        return Ok(());
    }

    let seed = Path::new(SEED_DIR);
    if !seed.is_dir() {
        return Ok(());
    }

    let seed_entries = fs::read_dir(seed)
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

/// App-specific scratch root under the OS temp directory (e.g.
/// `$TMPDIR/orquestra-ai`, `/tmp/orquestra-ai`). Never a hardcoded
/// `/var/tmp/testing`, which end-user machines do not have.
pub(crate) fn scratch_root() -> PathBuf {
    std::env::temp_dir().join("orquestra-ai")
}

/// Create a fresh unique scratch dir under [`scratch_root`] with `0o700`
/// permissions on Unix. The caller owns cleanup (`remove_dir_all`).
/// Uniqueness comes from pid + nanos + an atomic counter, so concurrent
/// imports/updates never share a staging tree.
pub(crate) fn fresh_scratch_dir(prefix: &str) -> Result<PathBuf, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let root = scratch_root();
    fs::create_dir_all(&root)
        .map_err(|error| format!("failed to create {}: {error}", root.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&root, fs::Permissions::from_mode(0o700));
    }
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = root.join(format!("{prefix}-{}-{unique}-{seq}", std::process::id()));
    fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create {}: {error}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}
