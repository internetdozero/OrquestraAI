//! Harness fusion: backing up each harness and linking it into its
//! OrquestraAI projection.
//!
//! [`status`] reports what a harness currently resolves and [`fuse`] performs
//! the backup-and-link work. This file exposes the two operations the desktop
//! shell's IPC commands and terminal flags call; nothing here touches Tauri.

use serde::Serialize;
use std::path::PathBuf;

use orquestra_core::paths::OrquestraPaths;

mod fuse;
mod status;
#[cfg(test)]
mod tests;

pub use orquestra_core::harness::SecondaryAction;

use fuse::*;
use status::*;

pub use fuse::timestamp_label;

/// Live state of one secondary discovery location.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecondaryStatus {
    pub path: String,
    pub action: SecondaryAction,
    /// True when the location can no longer feed the harness unmanaged skills.
    pub cleared: bool,
    pub rationale: String,
}

/// Live bridge state for one harness, consumed by the footer and modal.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessStatus {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub target: String,
    pub skills_paths: Vec<String>,
    pub skills_linked: bool,
    pub instructions_path: Option<String>,
    pub instructions_linked: bool,
    pub config_path: Option<String>,
    pub config_linked: bool,
    /// Secondary discovery locations this harness reads behind the projection.
    pub secondary_paths: Vec<SecondaryStatus>,
    /// True when every secondary location is claimed or archived.
    pub secondary_cleared: bool,
    pub installed: bool,
    pub supported: bool,
    pub bridged: bool,
    pub note: Option<String>,
}

/// Outcome of a fusion run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeReport {
    pub backup_dir: Option<String>,
    pub bridged: usize,
    pub total: usize,
    pub harnesses: Vec<HarnessStatus>,
    pub adopted: Vec<String>,
    /// Secondary discovery locations emptied or replaced during this run.
    pub quarantined: Vec<String>,
    pub instructions_imported: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// The operator's home directory: the harnesses live under it, so this is
/// exactly what [`orquestra_core::paths::require_home`] resolves.
pub fn home_dir() -> Result<PathBuf, String> {
    Ok(orquestra_core::paths::require_home(
        "locate harness directories",
    )?)
}

/// Current bridge state for every known harness.
pub fn get_bridge_status() -> Result<Vec<HarnessStatus>, String> {
    let paths = OrquestraPaths::resolve()?;
    let home = home_dir()?;
    Ok(statuses_at(&home, &paths))
}

/// Back up and then symlink every harness into its OrquestraAI projection.
pub fn deploy_harness_bridge() -> Result<BridgeReport, String> {
    let paths = OrquestraPaths::resolve()?;
    paths.bootstrap()?;
    let home = home_dir()?;
    deploy_at(&home, &paths)
}
