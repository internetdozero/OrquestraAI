//! Resolved storage locations for the vault, the projections and the
//! master instruction file, plus the first-run bootstrap that creates
//! them.
//!
//! [`home`] is the single place that knows which variables name the operator's
//! home directory; [`scratch`] owns the per-process runtime and staging
//! storage; [`seed`] owns first-launch seeding of an empty vault.

mod home;
mod scratch;
mod seed;

pub use home::{home_dir, home_dir_with, require_home, require_home_with, HOME_VARS};
pub use scratch::{ensure_scratch_root, fresh_scratch_dir, scratch_root, unique_tmp_suffix};
pub use seed::copy_dir_all;

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{CoreError, CoreResult};
use crate::harness::{harness_ids, GLOBAL_SCOPE};

pub const AGENTS_FILE: &str = "AGENTS.md";

/// Resolved storage locations for the single-source-of-truth vault, the
/// per-scope symlink projections, and the master instruction file.
#[derive(Debug, Clone)]
pub struct OrquestraPaths {
    pub root: PathBuf,
    pub vault: PathBuf,
    pub active: PathBuf,
}

impl OrquestraPaths {
    pub fn resolve() -> CoreResult<Self> {
        let root = require_home("locate ~/.orquestra")?.join(".orquestra");

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

pub fn open_paths() -> CoreResult<OrquestraPaths> {
    let paths = OrquestraPaths::resolve()?;
    paths.bootstrap()?;
    Ok(paths)
}
