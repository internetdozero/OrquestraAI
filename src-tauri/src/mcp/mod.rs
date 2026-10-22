//! Global and per-harness MCP server management.
//!
//! One master catalog lives in `~/.orquestra/mcps.json`; every harness wants
//! a different envelope around the same server definition, so
//! [`serializers`] is the adapter layer, [`deploy`] owns the atomic writes
//! and backups, and [`catalog`] owns the storage behind both. Every write
//! starts from the file already on disk, so keys OrquestraAI does not own
//! (project state, hooks, plugin tables, hand-added servers) survive
//! untouched.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod catalog;
mod deploy;
mod secrets;
mod serializers;
#[cfg(test)]
mod tests;

pub use catalog::{
    apply_global_selection, apply_harness_selections, batch_set_mcp_all, delete_mcp, list_mcps,
    list_mcps_masked, reveal_mcp_secret, save_mcp, set_mcp_state,
};

/// Zeroize and unlink the catalog's `.migrate-bak` plaintext backup. The
/// sealed catalog must still open under the current key, so this never
/// destroys the only readable copy; see `crate::at_rest`.
pub fn purge_catalog_migration_backup() -> Result<bool, String> {
    let paths = crate::paths::OrquestraPaths::resolve()?;
    let path = catalog::catalog_path(&paths);
    crate::at_rest::purge_migration_backup(&paths, &path, crate::at_rest::AAD_MCPS)
}
pub use deploy::{deploy_mcps, deploy_mcps_reporting, list_targets};
pub use secrets::mask_secret;

/// One MCP server as the operator defines it, independent of any harness.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    /// Projected into every harness that has an adapter.
    #[serde(default)]
    pub global_enabled: bool,
    /// Harnesses that opt in on top of the global baseline.
    #[serde(default)]
    pub enabled_harnesses: Vec<String>,
    /// Computed official tier (see `crate::official`); stamped on the way
    /// out and recomputed on save, never operator state.
    #[serde(default)]
    pub official: bool,
}

impl McpServer {
    pub(super) fn resolved(&self, dotenv: &HashMap<String, String>) -> Self {
        let mut resolved = self.clone();
        resolved.args = self
            .args
            .iter()
            .map(|value| secrets::interpolate(value, dotenv))
            .collect();
        resolved.env = self
            .env
            .iter()
            .map(|(key, value)| (key.clone(), secrets::interpolate(value, dotenv)))
            .collect();
        resolved.url = self
            .url
            .as_ref()
            .map(|value| secrets::interpolate(value, dotenv));
        resolved.headers = self
            .headers
            .iter()
            .map(|(key, value)| (key.clone(), secrets::interpolate(value, dotenv)))
            .collect();
        resolved
    }
}

impl McpServer {
    /// Enabled for a specific harness, directly or through the global baseline.
    pub(super) fn covers(&self, harness: &str) -> bool {
        self.global_enabled || self.enabled_harnesses.iter().any(|id| id == harness)
    }

    pub(super) fn active_anywhere(&self) -> bool {
        self.global_enabled || !self.enabled_harnesses.is_empty()
    }
}

/// Live state of one harness config OrquestraAI can write.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTargetStatus {
    pub id: String,
    pub label: String,
    pub config_path: String,
    pub installed: bool,
}
