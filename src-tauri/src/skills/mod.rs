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
mod import;
mod projection;
mod security;
mod subpath;
mod vault;
mod vault_cache;

pub(crate) use subpath::refresh_skill_dir;
#[cfg(test)]
mod tests;

pub(crate) use agents::{read_agents, write_agents, DEFAULT_AGENTS};
pub use agents::{read_master_agents_md, write_master_agents_md};
pub use import::{import_skill, import_skill_from_git};
pub(crate) use vault::SKILL_FILE;

use projection::{batch_from, clear_global_baseline, projected_ids, set_global, set_scope};
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
    /// Computed official tier (see `crate::official`); recomputed on every
    /// listing, never operator state.
    #[serde(default)]
    pub official: bool,
    /// Static security audit: true when the skill folder holds helper
    /// scripts (`.sh`, `.py`, `.js`, …) or a `scripts/`/`bin/` payload.
    #[serde(default)]
    pub has_scripts: bool,
    /// Repository-relative script paths, sorted for stable snapshots.
    #[serde(default)]
    pub script_files: Vec<String>,
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
    // One `read_dir` per scope: the old loop paid two `canonicalize`
    // calls per skill per scope (see `projected_ids`).
    let global = projected_ids(&paths.global_dir(), &paths.vault);
    let global_active = directories
        .iter()
        .filter(|(id, _)| global.contains(id))
        .count();
    let mut harnesses: Vec<(&str, usize)> =
        harness_ids().map(|harness| (harness, 0usize)).collect();

    for (harness, active) in harnesses.iter_mut() {
        let scoped = projected_ids(&paths.projection_dir(harness), &paths.vault);
        *active = directories
            .iter()
            .filter(|(id, _)| global.contains(id) || scoped.contains(id))
            .count();
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

/// Open a native OS folder picker and return the chosen directory.
/// `Ok(None)` means the user cancelled. Linux tries `zenity`, then
/// `kdialog`; when neither exists an `Err` tells the UI to fall back to
/// the typed path instead of failing silently.
pub async fn pick_folder() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(run_folder_picker)
        .await
        .map_err(|error| format!("folder picker task failed: {error}"))?
}

fn run_folder_picker() -> Result<Option<String>, String> {
    const TITLE: &str = "Selecione a pasta da Skill";

    // Owned args per backend: zenity first, then kdialog.
    let owned: Vec<(String, Vec<String>)> = vec![
        (
            "zenity".to_string(),
            vec![
                "--file-selection".to_string(),
                "--directory".to_string(),
                format!("--title={TITLE}"),
            ],
        ),
        (
            "kdialog".to_string(),
            vec![
                "--getexistingdirectory".to_string(),
                ".".to_string(),
                "--title".to_string(),
                TITLE.to_string(),
            ],
        ),
    ];
    for (program, args) in &owned {
        let output = match std::process::Command::new(program).args(args).output() {
            Ok(output) => output,
            // Backend missing: try the next one.
            Err(_) => continue,
        };
        // Non-zero exit is the user cancelling the dialog.
        if !output.status.success() {
            return Ok(None);
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            return Ok(None);
        }
        return Ok(Some(path));
    }

    Err(
        "no folder picker available (tried zenity, kdialog); type or paste the path instead"
            .to_string(),
    )
}

/// Clear only the Global baseline link for one skill, preserving harness
/// projections. Kept for harness-specific workflows; preset reconciliation
/// now disables globally so orphan harness links are swept too.
#[allow(dead_code)]
pub fn clear_global_baseline_for_preset(id: &str) -> Result<(), String> {
    let paths = open_paths()?;
    clear_global_baseline(&paths, id)
}

/// Read the raw `SKILL.md` of one vault skill, for the in-app editor.
pub fn read_skill_content(id: String) -> Result<String, String> {
    let paths = open_paths()?;
    vault::read_skill_content(&paths, &id)
}

/// Atomically replace a skill's `SKILL.md`.
pub fn save_skill_content(id: String, content: String) -> Result<(), String> {
    let paths = open_paths()?;
    vault::save_skill_content(&paths, &id, &content)
}

/// Unlink a skill from every scope and remove its vault folder.
pub fn delete_skill(id: String) -> Result<(), String> {
    let paths = open_paths()?;
    vault::delete_skill(&paths, &id)
}

fn validate_scope(scope: &str) -> Result<(), String> {
    if scope == GLOBAL_SCOPE || harness::find(scope).is_some() {
        Ok(())
    } else {
        Err(format!("unknown harness scope: {scope}"))
    }
}
