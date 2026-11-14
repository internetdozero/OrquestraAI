//! Resolved storage locations for the vault, the projections and the
//! master instruction file, plus the first-run bootstrap that creates
//! them.
//!
//! [`home`] is the single place that knows which variables name the operator's
//! home directory; [`dirs`] owns the per-OS data and config roots and the
//! rebasing of home-relative harness paths; [`scratch`] owns the per-process
//! runtime and staging storage; [`seed`] owns first-launch seeding of an empty
//! vault.

mod dirs;
mod home;
mod scratch;
mod seed;

pub use dirs::{config_dir, config_dir_for, data_dir, data_dir_for, rebase, rebase_for};
pub use home::{home_dir, home_dir_with, require_home, require_home_with, HOME_VARS};
pub use scratch::{ensure_scratch_root, fresh_scratch_dir, scratch_root, unique_tmp_suffix};
pub use seed::copy_dir_all;

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{CoreError, CoreResult};
use crate::harness::{harness_ids, GLOBAL_SCOPE};

pub const AGENTS_FILE: &str = "AGENTS.md";

/// Pre-OwnHub storage root name. The only place the old name survives: a
/// first run after the rename moves `~/.orquestra` to `~/.ownhub` in place
/// (same home, same filesystem, so `rename` is atomic) instead of
/// orphaning the vault, projections and backups on disk.
const LEGACY_ROOT_NAME: &str = ".orquestra";

/// The OwnHub storage root under `home`, migrating the legacy root first.
///
/// Only a missing new root with an existing legacy directory is moved;
/// when both exist the new one wins and nothing is merged or overwritten.
/// A failed move is best-effort: the caller still gets the new root and
/// `ensure_layout` recreates the layout there.
pub fn storage_root(home: &Path) -> PathBuf {
    let root = home.join(".ownhub");
    let legacy = home.join(LEGACY_ROOT_NAME);
    if !root.exists() && legacy.is_dir() {
        let _ = fs::rename(&legacy, &root);
    }
    root
}

/// Resolved storage locations for the single-source-of-truth vault, the
/// per-scope symlink projections, and the master instruction file.
#[derive(Debug, Clone)]
pub struct OwnHubPaths {
    pub root: PathBuf,
    pub vault: PathBuf,
    pub active: PathBuf,
}

impl OwnHubPaths {
    pub fn resolve() -> CoreResult<Self> {
        let root = storage_root(&require_home("locate ~/.ownhub")?);

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
    pub fn ensure_layout(&self) -> CoreResult<()> {
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
    pub fn bootstrap(&self) -> CoreResult<()> {
        self.ensure_layout()?;
        seed::seed_vault(&self.root, &self.vault)
    }
}

pub fn create_dir(path: &Path) -> CoreResult<()> {
    fs::create_dir_all(path).map_err(|source| CoreError::io("create", path, source))
}

pub fn open_paths() -> CoreResult<OwnHubPaths> {
    let paths = OwnHubPaths::resolve()?;
    paths.bootstrap()?;
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let home = std::env::temp_dir().join(format!(
            "ownhub-migrate-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&home).expect("home dir");
        home
    }

    #[test]
    fn legacy_root_is_moved_to_the_new_root() {
        let home = temp_home("move");
        std::fs::create_dir_all(home.join(".orquestra/skills-library")).expect("legacy dir");
        std::fs::write(home.join(".orquestra/AGENTS.md"), "hi\n").expect("legacy file");

        let root = storage_root(&home);

        assert_eq!(root, home.join(".ownhub"));
        assert!(home.join(".ownhub/AGENTS.md").is_file());
        assert!(!home.join(".orquestra").exists());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn an_existing_new_root_is_never_overwritten() {
        let home = temp_home("keep");
        std::fs::create_dir_all(home.join(".ownhub")).expect("new dir");
        std::fs::write(home.join(".ownhub/AGENTS.md"), "new\n").expect("new file");
        std::fs::create_dir_all(home.join(".orquestra")).expect("legacy dir");
        std::fs::write(home.join(".orquestra/AGENTS.md"), "old\n").expect("legacy file");

        let root = storage_root(&home);

        assert_eq!(root, home.join(".ownhub"));
        assert_eq!(
            std::fs::read_to_string(home.join(".ownhub/AGENTS.md")).expect("new file"),
            "new\n"
        );
        assert!(home.join(".orquestra/AGENTS.md").is_file());
        let _ = std::fs::remove_dir_all(&home);
    }
}
