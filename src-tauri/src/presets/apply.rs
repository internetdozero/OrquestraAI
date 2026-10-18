//! Applying a preset at the two layers it owns.
//!
//! Skills reconcile in two passes: the universal Global baseline first
//! (clearing Global propagates into every harness folder, sweeping orphans),
//! then each harness's own specialization links on top. MCP servers reconcile
//! the same way in the master catalog — global baseline plus per-harness
//! opt-ins — then compile into the harness configs and refresh the bridge.

use std::collections::{BTreeSet, HashMap};

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
    let (harness_skills, harness_skill_warnings) =
        super::scopes::plan_scope_map(&preset.harness_skills, &known_skills, "skill");
    let (harness_mcps, harness_mcp_warnings) =
        super::scopes::plan_scope_map(&preset.harness_mcps, &known_mcps, "mcp");
    let mut warnings: Vec<String> = skill_warnings;
    warnings.extend(mcp_warnings);
    warnings.extend(harness_skill_warnings);
    warnings.extend(harness_mcp_warnings);

    let reconcile_warnings = reconcile_skills(&skills, preset.all_skills)?;
    warnings.extend(reconcile_warnings);
    if !preset.all_skills {
        warnings.extend(reconcile_harness_skills(&skills, &harness_skills)?);
    }
    crate::mcp::apply_global_selection(&mcps.iter().cloned().collect::<Vec<_>>())?;
    if !preset.all_mcps {
        // Authoritative per harness: a server left out of a harness list is
        // removed from that harness. Planned (known-only) maps only.
        let filtered: HashMap<String, Vec<String>> = harness_mcps
            .iter()
            .map(|(harness, set)| {
                (harness.clone(), set.iter().cloned().collect::<Vec<_>>())
            })
            .collect();
        crate::mcp::apply_harness_selections(&filtered)?;
    }

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

    let all_skills = super::scopes::union_unique(&skills, &harness_skills);
    let all_mcps = super::scopes::union_unique(&mcps, &harness_mcps);
    Ok(PresetReport {
        id: preset.id.clone(),
        name: preset.name.clone(),
        skills_enabled: all_skills.len(),
        skills_disabled: known_skills.len().saturating_sub(all_skills.len()),
        mcps_enabled: all_mcps.len(),
        mcps_disabled: known_mcps.len().saturating_sub(all_mcps.len()),
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

/// Project exactly `wanted` in Global using delta reconciliation:
/// clears the Global baseline link for skills that are no longer wanted and
/// also unlinks those ids from every harness projection folder, so a skill
/// dropped from a preset cannot linger as an orphan symlink. Newly wanted
/// skills are linked in Global, which propagates through the projections.
/// Collects partial warnings without aborting.
pub(super) fn reconcile_skills(
    wanted: &BTreeSet<String>,
    all_skills: bool,
) -> Result<Vec<String>, String> {
    let currently_active: BTreeSet<String> = crate::skills::list_skills(None)?
        .into_iter()
        .filter(|skill| skill.global_enabled)
        .map(|skill| skill.id)
        .collect();

    let to_disable: Vec<String> = if all_skills {
        Vec::new()
    } else {
        currently_active.difference(wanted).cloned().collect()
    };
    let to_enable: Vec<String> = wanted.difference(&currently_active).cloned().collect();

    let mut warnings = Vec::new();

    for id in &to_disable {
        // Global disable propagates into every harness folder, removing both
        // the baseline and any propagated links in one pass.
        if let Err(error) = crate::skills::set_skill_state(id, GLOBAL_SCOPE, false) {
            warnings.push(format!("failed to disable skill “{id}”: {error}"));
        }
    }

    if !all_skills {
        // Sweep harness-only orphans: skills never in the Global baseline
        // but still linked inside a harness projection folder.
        for harness in crate::harness::harness_ids() {
            let harness_active = match crate::skills::list_skills(Some(harness.to_string())) {
                Ok(list) => list,
                Err(error) => {
                    warnings.push(format!("failed to read harness “{harness}”: {error}"));
                    continue;
                }
            };
            for skill in harness_active {
                if !skill.harness_enabled || wanted.contains(&skill.id) {
                    continue;
                }
                if let Err(error) =
                    crate::skills::set_skill_state(&skill.id, harness, false)
                {
                    // Inherited skills were already cleared by the Global
                    // disable above; anything else is a real failure.
                    if !error.contains("inherited from Global") {
                        warnings.push(format!(
                            "failed to disable skill “{}” in “{harness}”: {error}",
                            skill.id
                        ));
                    }
                }
            }
        }
    }

    for id in &to_enable {
        if let Err(error) = crate::skills::set_skill_state(id, GLOBAL_SCOPE, true) {
            warnings.push(format!("failed to enable skill “{id}”: {error}"));
        }
    }

    Ok(warnings)
}

/// Reconcile per-harness specialization on top of the Global baseline.
/// `global` is the planned Global set; `wanted` maps each harness to its
/// planned harness-only set. Links Global already projects need no action;
/// harness-only links missing from `wanted` are unlinked, and missing wanted
/// links are created. Unknown harness scopes in `wanted` are ignored.
pub(super) fn reconcile_harness_skills(
    global: &BTreeSet<String>,
    wanted: &HashMap<String, BTreeSet<String>>,
) -> Result<Vec<String>, String> {
    let mut warnings = Vec::new();

    for harness in crate::harness::harness_ids() {
        let empty = BTreeSet::new();
        let wanted_harness = wanted.get(harness).unwrap_or(&empty);
        let current = match crate::skills::list_skills(Some(harness.to_string())) {
            Ok(list) => list,
            Err(error) => {
                warnings.push(format!("failed to read harness “{harness}”: {error}"));
                continue;
            }
        };

        for skill in &current {
            // Only harness-only links are owned here; inherited ones were
            // handled by the Global pass.
            if !skill.harness_enabled || skill.global_enabled {
                continue;
            }
            if global.contains(&skill.id) || wanted_harness.contains(&skill.id) {
                continue;
            }
            if let Err(error) = crate::skills::set_skill_state(&skill.id, harness, false) {
                if !error.contains("inherited from Global") {
                    warnings.push(format!(
                        "failed to disable skill “{}” in “{harness}”: {error}",
                        skill.id
                    ));
                }
            }
        }

        for id in wanted_harness {
            if global.contains(id) {
                continue;
            }
            let already = current
                .iter()
                .any(|skill| skill.id == *id && skill.harness_enabled);
            if already {
                continue;
            }
            if let Err(error) = crate::skills::set_skill_state(id, harness, true) {
                warnings.push(format!(
                    "failed to enable skill “{id}” in “{harness}”: {error}"
                ));
            }
        }
    }

    Ok(warnings)
}
