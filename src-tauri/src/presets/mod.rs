//! Mission presets: curated skill and MCP profiles applied in one command.
//!
//! A preset is a named snapshot of which skills and MCP servers should be
//! live. [`builtins`] holds the profiles shipped with the app, [`store`] owns
//! `~/.orquestra/presets.json` for the operator's own profiles, and [`apply`]
//! reconciles the skill projections and the MCP catalog in one pass.
//!
//! This file keeps the API the Tauri layer and the terminal call, plus the
//! shapes it serializes.

use serde::{Deserialize, Serialize};

mod apply;
mod builtins;
mod store;
#[cfg(test)]
mod tests;

pub use apply::apply_preset;
pub use store::{delete_preset, list_presets, save_preset};

/// One named profile of active skills and MCP servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Icon key the client maps to a glyph (`shield`, `cpu`, `globe`, `zap`).
    #[serde(default = "default_icon")]
    pub icon: String,
    /// Skill ids to project in the universal baseline.
    #[serde(default)]
    pub skills: Vec<String>,
    /// MCP server ids to enable globally.
    #[serde(default)]
    pub mcps: Vec<String>,
    /// Ignore `skills` and project the whole vault.
    #[serde(default)]
    pub all_skills: bool,
    /// Ignore `mcps` and enable the whole catalog.
    #[serde(default)]
    pub all_mcps: bool,
    /// Shipped profiles cannot be deleted.
    #[serde(default)]
    pub builtin: bool,
    /// True for the profile most recently applied.
    #[serde(default)]
    pub active: bool,
}

fn default_icon() -> String {
    "custom".to_string()
}

/// What an apply changed, for the toast and the headless report.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetReport {
    pub id: String,
    pub name: String,
    pub skills_enabled: usize,
    pub skills_disabled: usize,
    pub mcps_enabled: usize,
    pub mcps_disabled: usize,
    /// Harness config files re-compiled by the MCP deploy.
    pub configs_changed: Vec<String>,
    /// Non-fatal problems: unknown ids, a partial deploy, a bridge that had to
    /// be refreshed but could not be.
    pub warnings: Vec<String>,
}

/// Guard against ids that could escape the store or collide with path syntax.
pub(super) fn validate_id(id: &str) -> Result<(), String> {
    let invalid = id.is_empty()
        || id == "."
        || id == ".."
        || id.starts_with('.')
        || id.contains('/')
        || id.contains('\\')
        || id.contains('\0')
        || id.chars().any(char::is_whitespace);

    if invalid {
        Err(format!("invalid preset id: {id:?}"))
    } else {
        Ok(())
    }
}
