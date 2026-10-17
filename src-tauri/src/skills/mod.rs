//! Skill administration: the master vault, its per-scope projections, and
//! the master `AGENTS.md`.
//!
//! Split by responsibility — [`vault`] indexes `SKILL.md` files, [`projection`]
//! owns the symlinks under `~/.orquestra/active`, and [`agents`] owns the
//! instruction file. This file keeps the API the Tauri layer calls and the
//! shapes it serializes.

use serde::{Deserialize, Serialize};

use crate::harness::{self, harness_ids, GLOBAL_SCOPE};
use crate::paths::{open_paths, OrquestraPaths};

mod agents;
mod projection;
mod vault;
#[cfg(test)]
mod tests;

pub(crate) use agents::{read_agents, write_agents, DEFAULT_AGENTS};
pub use agents::{read_master_agents_md, write_master_agents_md};
pub(crate) use vault::SKILL_FILE;

use projection::{batch_from, is_projected, set_global, set_scope};
use vault::{list_from, skill_directories};

/// A skill discovered in the master vault, evaluated for one selected scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub tokens: usize,
    /// Whether the skill is projected in the universal baseline.
    pub global_enabled: bool,
    /// Effective state for the selected harness tab.
    pub harness_enabled: bool,
    /// True when the harness state is forced ON by the global baseline.
    pub inherited_from_global: bool,
}

/// Active-skill counters for the sidebar, one row per scope.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessSummary {
    pub id: String,
    pub active: usize,
    pub total: usize,
}

/// Read every skill folder and evaluate it for the requested harness scope.
/// `None` or `Some("global")` evaluates the universal baseline.
pub fn list_skills(harness_id: Option<String>) -> Result<Vec<SkillItem>, String> {
    let paths = open_paths()?;
    let scope = harness_id.unwrap_or_else(|| GLOBAL_SCOPE.to_string());
    validate_scope(&scope)?;
    list_from(&paths, &scope)
}

fn summary_from(paths: &OrquestraPaths) -> Result<Vec<HarnessSummary>, String> {
    let directories = skill_directories(&paths.vault)?;
    let total = directories.len();
    let global_dir = paths.global_dir();
    let mut global_active = 0usize;
    let mut harnesses: Vec<(&str, usize)> =
        harness_ids().map(|harness| (harness, 0usize)).collect();

    for (id, _) in &directories {
        let is_global = is_projected(&global_dir, &paths.vault, id);
        if is_global {
            global_active += 1;
        }

        for (harness, active) in harnesses.iter_mut() {
            if is_global || is_projected(&paths.projection_dir(harness), &paths.vault, id) {
                *active += 1;
            }
        }
    }

    let mut summaries = Vec::with_capacity(harnesses.len() + 1);
    summaries.push(HarnessSummary {
        id: GLOBAL_SCOPE.to_string(),
        active: global_active,
        total,
    });
    for (harness, active) in harnesses {
        summaries.push(HarnessSummary {
            id: harness.to_string(),
            active,
            total,
        });
    }

    Ok(summaries)
}

/// Active-skill counters for Global and every harness, used by the sidebar.
pub fn harness_summary() -> Result<Vec<HarnessSummary>, String> {
    let paths = open_paths()?;
    summary_from(&paths)
}

/// Project or un-project a skill for one scope. Updating Global propagates the
/// symlink into every harness folder; harness updates stay isolated.
pub fn set_skill_state(id: &str, harness_id: &str, enabled: bool) -> Result<(), String> {
    let paths = open_paths()?;
    validate_scope(harness_id)?;

    if harness_id == GLOBAL_SCOPE {
        set_global(&paths, id, enabled)
    } else {
        set_scope(&paths, id, harness_id, enabled)
    }
}

/// Toggle every skill for one scope. Harness-wide disable leaves inherited
/// Global skills untouched.
pub fn batch_set_all(harness_id: String, enabled: bool) -> Result<(), String> {
    let paths = open_paths()?;
    validate_scope(&harness_id)?;
    batch_from(&paths, &harness_id, enabled)
}

fn validate_scope(scope: &str) -> Result<(), String> {
    if scope == GLOBAL_SCOPE || harness::find(scope).is_some() {
        Ok(())
    } else {
        Err(format!("unknown harness scope: {scope}"))
    }
}
