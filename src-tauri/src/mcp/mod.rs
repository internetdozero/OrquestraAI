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
mod serializers;
#[cfg(test)]
mod tests;

pub use catalog::{apply_global_selection, delete_mcp, list_mcps, save_mcp, set_mcp_state};
pub use deploy::{deploy_mcps, deploy_mcps_reporting, list_targets};

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
    /// Projected into every harness that has an adapter.
    #[serde(default)]
    pub global_enabled: bool,
    /// Harnesses that opt in on top of the global baseline.
    #[serde(default)]
    pub enabled_harnesses: Vec<String>,
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
