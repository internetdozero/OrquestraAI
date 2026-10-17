//! Applying a preset at the two layers it owns.
//!
//! Skills are reconciled through the universal Global projection: clearing
//! Global removes the inherited and harness-only links alike, then projecting
//! the chosen set recreates them everywhere, so every harness converges on the
//! same profile. MCP servers are reconciled in the master catalog, then
//! compiled into the harness configs and (when needed) the harness bridge is
//! refreshed so the folders actually resolve through the projection.

use std::collections::BTreeSet;

use super::{store, Preset, PresetReport};
use crate::harness::GLOBAL_SCOPE;

/// Apply `id` and report what changed. Unknown skill or MCP ids are ignored
/// with a warning: a preset stays usable on a machine whose vault differs.
pub fn apply_preset(id: String) -> Result<PresetReport, String> {
    let file = store::load_current()?;
    let preset = store::find(&file, &id).ok_or_else(|| format!("unknown preset: {id}"))?;

    let known_skills: Vec<String> = crate::skills::list_skills(None)?
        .into_iter()
        .map(|skill| skill.id)
        .collect();
    let known_mcps: Vec<String> = crate::mcp::list_mcps()?
        .into_iter()
        .map(|server| server.id)
        .collect();

    let (skills, skill_warnings) = plan_skills(&preset, &known_skills);
    let (mcps, mcp_warnings) = plan_mcps(&preset, &known_mcps);
    let mut warnings: Vec<String> = skill_warnings;
    warnings.extend(mcp_warnings);

    reconcile_skills(&skills, preset.all_skills)?;
    crate::mcp::apply_global_selection(&mcps.iter().cloned().collect::<Vec<_>>())?;

    let configs_changed = match crate::mcp::deploy_mcps_reporting() {
        Ok(changed) => changed,
        Err(error) => {
            warnings.push(format!("MCP deploy incomplete: {error}"));
            Vec::new()
        }
    };

    // Harnesses resolve skills through their projection symlinks; this is a
    // no-op once the bridge is already deployed.
    if let Err(error) = crate::bridge::deploy_harness_bridge() {
        warnings.push(format!("harness bridge not refreshed: {error}"));
    }

    store::remember_active(&preset.id)?;

    Ok(PresetReport {
        id: preset.id.clone(),
        name: preset.name.clone(),
        skills_enabled: skills.len(),
        skills_disabled: known_skills.len().saturating_sub(skills.len()),
        mcps_enabled: mcps.len(),
        mcps_disabled: known_mcps.len().saturating_sub(mcps.len()),
        configs_changed,
        warnings,
    })
}

/// The skills a preset resolves to against this vault, plus a warning per
/// requested id that is not installed.
pub(super) fn plan_skills(preset: &Preset, known: &[String]) -> (BTreeSet<String>, Vec<String>) {
    if preset.all_skills {
        return (known.iter().cloned().collect(), Vec::new());
    }

    let installed: BTreeSet<&str> = known.iter().map(String::as_str).collect();
    let mut wanted = BTreeSet::new();
    let mut warnings = Vec::new();

    for id in &preset.skills {
        if installed.contains(id.as_str()) {
            wanted.insert(id.clone());
        } else {
            warnings.push(format!("skill “{id}” is not in the vault"));
        }
    }

    (wanted, warnings)
}

/// The MCP ids a preset resolves to against this catalog, plus a warning per
/// requested id that is not installed.
pub(super) fn plan_mcps(preset: &Preset, known: &[String]) -> (BTreeSet<String>, Vec<String>) {
    if preset.all_mcps {
        return (known.iter().cloned().collect(), Vec::new());
    }

    let installed: BTreeSet<&str> = known.iter().map(String::as_str).collect();
    let mut wanted = BTreeSet::new();
    let mut warnings = Vec::new();

    for id in &preset.mcps {
        if installed.contains(id.as_str()) {
            wanted.insert(id.clone());
        } else {
            warnings.push(format!("MCP server “{id}” is not in the catalog"));
        }
    }

    (wanted, warnings)
}

/// Project exactly `wanted` in Global. Disabling Global also unlinks every
/// harness-only projection, which is what makes a two-skill preset actually
/// replace a 400-skill session.
fn reconcile_skills(wanted: &BTreeSet<String>, all_skills: bool) -> Result<(), String> {
    if all_skills {
        return crate::skills::batch_set_all(GLOBAL_SCOPE.to_string(), true);
    }

    crate::skills::batch_set_all(GLOBAL_SCOPE.to_string(), false)?;
    for id in wanted {
        crate::skills::set_skill_state(id, GLOBAL_SCOPE, true)?;
    }

    Ok(())
}
