//! Persistence for custom presets in `~/.orquestra/presets.json`.
//!
//! The file only holds operator-created profiles plus which preset was applied
//! last; the built-ins are compiled in. A missing file is normal on first run
//! and simply means "no custom presets", so the store always falls back to the
//! built-in set.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{builtins, validate_id, Preset};
use crate::paths::OrquestraPaths;

/// Custom preset file inside `~/.orquestra`.
const PRESETS_FILE: &str = "presets.json";

/// On-disk shape: the operator's profiles, overrides for built-ins, and the applied one.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PresetFile {
    #[serde(default)]
    pub(super) custom: Vec<Preset>,
    #[serde(default)]
    pub(super) active: Option<String>,
    #[serde(default)]
    pub(super) overrides: Vec<Preset>,
}

impl PresetFile {
    #[cfg(test)]
    pub(super) fn custom(&self) -> &[Preset] {
        &self.custom
    }

    #[cfg(test)]
    pub(super) fn overrides(&self) -> &[Preset] {
        &self.overrides
    }

    pub(super) fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Built-ins plus custom profiles, with the applied one flagged for the bar.
pub fn list_presets() -> Result<Vec<Preset>, String> {
    let file = load_current()?;
    Ok(merged(&file))
}

/// Snapshot the live combination of skills and MCPs as a new custom profile.
pub fn save_preset(name: String, description: Option<String>) -> Result<Preset, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("a preset needs a name".to_string());
    }

    let root = root()?;
    let mut file = load_from(&root)?;
    let (skills, mcps, harness_skills, harness_mcps) = capture_live_state()?;
    let id = unique_id(&name, &file);

    let preset = Preset {
        id: id.clone(),
        name,
        description: description.unwrap_or_default().trim().to_string(),
        icon: "custom".to_string(),
        skills,
        mcps,
        harness_skills,
        harness_mcps,
        all_skills: false,
        all_mcps: false,
        builtin: false,
        active: true,
    };

    file.custom.retain(|existing| existing.id != id);
    file.custom.push(preset.clone());
    file.active = Some(id);
    save_to(&root, &file)?;

    Ok(preset)
}

/// Remove one custom profile. Built-ins are compiled in and cannot be deleted.
pub fn delete_preset(id: &str) -> Result<(), String> {
    if builtins::all().iter().any(|preset| preset.id == id) {
        return Err(format!("“{id}” ships with OrquestraAI and cannot be deleted"));
    }

    let root = root()?;
    let mut file = load_from(&root)?;
    let before = file.custom.len();
    file.custom.retain(|preset| preset.id != id);
    if file.custom.len() == before {
        return Err(format!("unknown preset: {id}"));
    }

    if file.active.as_deref() == Some(id) {
        file.active = None;
    }

    save_to(&root, &file)
}

/// Update an existing preset.
///
/// If `preset.id` matches a built-in profile, user customizations are saved to
/// `file.overrides` while keeping `builtin: true`.
/// If `preset.id` is custom, it is updated in `file.custom`.
pub fn update_preset(mut preset: Preset) -> Result<Preset, String> {
    validate_id(&preset.id)?;
    let name = preset.name.trim().to_string();
    if name.is_empty() {
        return Err("a preset needs a name".to_string());
    }
    preset.name = name;
    preset.description = preset.description.trim().to_string();
    // Normalize scope maps so hand-edited payloads cannot store unknown
    // harnesses, empties, or path-shaped ids; the edit modal spreads the
    // preset, so untouched per-harness scopes survive an update as-is.
    preset.harness_skills = super::scopes::normalize_scope_map(&preset.harness_skills);
    preset.harness_mcps = super::scopes::normalize_scope_map(&preset.harness_mcps);

    let root = root()?;
    let mut file = load_from(&root)?;

    let is_builtin = builtins::all().iter().any(|b| b.id == preset.id);
    if is_builtin {
        preset.builtin = true;
        file.overrides.retain(|existing| existing.id != preset.id);
        file.overrides.push(preset.clone());
    } else {
        preset.builtin = false;
        let before = file.custom.len();
        file.custom.retain(|existing| existing.id != preset.id);
        if file.custom.len() == before {
            return Err(format!("unknown preset: {}", preset.id));
        }
        file.custom.push(preset.clone());
    }

    save_to(&root, &file)?;

    // Return preset with accurate active flag
    preset.active = file.active.as_deref() == Some(preset.id.as_str());
    Ok(preset)
}

/// Reset a built-in preset to its factory defaults by discarding its override.
pub fn reset_preset(id: &str) -> Result<Preset, String> {
    let original = builtins::all()
        .into_iter()
        .find(|b| b.id == id)
        .ok_or_else(|| format!("“{id}” is not a built-in preset"))?;

    let root = root()?;
    let mut file = load_from(&root)?;
    file.overrides.retain(|existing| existing.id != id);
    save_to(&root, &file)?;

    let mut preset = original;
    preset.active = file.active.as_deref() == Some(id);
    Ok(preset)
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

pub(super) fn load_current() -> Result<PresetFile, String> {
    load_from(&root()?)
}

/// Built-ins (with overrides applied) first, then custom profiles, with `active`
/// recomputed from the stored id so the flag can never drift from what was actually applied.
pub(super) fn merged(file: &PresetFile) -> Vec<Preset> {
    let active = file.active();

    let mut presets = builtins::all();
    for builtin in presets.iter_mut() {
        if let Some(ov) = file.overrides.iter().find(|o| o.id == builtin.id) {
            builtin.name = ov.name.clone();
            builtin.description = ov.description.clone();
            builtin.icon = ov.icon.clone();
            builtin.skills = ov.skills.clone();
            builtin.mcps = ov.mcps.clone();
            builtin.harness_skills = ov.harness_skills.clone();
            builtin.harness_mcps = ov.harness_mcps.clone();
            builtin.all_skills = ov.all_skills;
            builtin.all_mcps = ov.all_mcps;
        }
    }

    for stored in &file.custom {
        let mut preset = stored.clone();
        preset.builtin = false;
        preset.active = false;
        presets.push(preset);
    }

    for preset in presets.iter_mut() {
        preset.active = active == Some(preset.id.as_str());
    }

    presets
}

/// One profile by id, from the built-ins or the custom list.
pub(super) fn find(file: &PresetFile, id: &str) -> Option<Preset> {
    merged(file).into_iter().find(|preset| preset.id == id)
}

/// Record which profile is active without touching the profile itself.
pub(super) fn remember_active(id: &str) -> Result<(), String> {
    validate_id(id)?;

    let root = root()?;
    let mut file = load_from(&root)?;
    if find(&file, id).is_none() {
        return Err(format!("unknown preset: {id}"));
    }

    file.active = Some(id.to_string());
    save_to(&root, &file)
}

fn root() -> Result<PathBuf, String> {
    Ok(OrquestraPaths::resolve()?.root)
}

fn file_path(root: &Path) -> PathBuf {
    root.join(PRESETS_FILE)
}

pub(super) fn load_from(root: &Path) -> Result<PresetFile, String> {
    let path = file_path(root);
    match fs::read_to_string(&path) {
        Ok(content) if !content.trim().is_empty() => serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse {}: {error}", path.display())),
        Ok(_) => Ok(PresetFile::default()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(PresetFile::default()),
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}

/// Atomic write: a sibling temp file is renamed over the target, so a crash
/// never leaves a truncated preset store behind.
pub(super) fn save_to(root: &Path, file: &PresetFile) -> Result<(), String> {
    let path = file_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    let mut content = serde_json::to_string_pretty(file)
        .map_err(|error| format!("failed to serialize presets: {error}"))?;
    content.push('\n');

    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary, &content)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;

    if let Err(error) = fs::rename(&temporary, &path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("failed to replace {}: {error}", path.display()));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Capture and naming
// ---------------------------------------------------------------------------

/// The Global baseline plus per-harness specialization, as the operator
/// currently sees it. Skills use the Global baseline for `skills` and only
/// harness-only links (enabled in a harness but not global) for
/// `harness_skills`; MCPs split `global_enabled` from `enabled_harnesses`.
/// A loose union would bundle orphan links as ghost entries.
fn capture_live_state() -> Result<
    (
        Vec<String>,
        Vec<String>,
        std::collections::HashMap<String, Vec<String>>,
        std::collections::HashMap<String, Vec<String>>,
    ),
    String,
> {
    use std::collections::{BTreeSet, HashMap};

    let mut global_skills = BTreeSet::new();
    for skill in crate::skills::list_skills(None)? {
        if skill.global_enabled {
            global_skills.insert(skill.id);
        }
    }

    // Harness-only links per harness, excluding anything Global already
    // projects (inherited skills belong to the baseline, not the harness).
    let mut harness_skills: HashMap<String, Vec<String>> = HashMap::new();
    for harness in crate::harness::harness_ids() {
        let mut only: Vec<String> = crate::skills::list_skills(Some(harness.to_string()))?
            .into_iter()
            .filter(|skill| skill.harness_enabled && !skill.global_enabled)
            .map(|skill| skill.id)
            .collect();
        only.sort();
        only.dedup();
        if !only.is_empty() {
            harness_skills.insert(harness.to_string(), only);
        }
    }

    let mut global_mcps = BTreeSet::new();
    let mut harness_mcps: HashMap<String, Vec<String>> = HashMap::new();
    for server in crate::mcp::list_mcps()? {
        if server.global_enabled {
            global_mcps.insert(server.id.clone());
        }
        for harness in &server.enabled_harnesses {
            harness_mcps
                .entry(harness.clone())
                .or_insert_with(Vec::new)
                .push(server.id.clone());
        }
    }
    for ids in harness_mcps.values_mut() {
        ids.sort();
        ids.dedup();
    }

    Ok((
        global_skills.into_iter().collect(),
        global_mcps.into_iter().collect(),
        harness_skills,
        harness_mcps,
    ))
}

/// `Game Dev` → `game-dev`; runs of punctuation collapse to one dash and a
/// name with no usable characters becomes `preset`.
pub(super) fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut separator_pending = false;

    for character in name.trim().to_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            if separator_pending && !slug.is_empty() {
                slug.push('-');
            }
            separator_pending = false;
            slug.push(character);
        } else {
            separator_pending = true;
        }
    }

    if slug.is_empty() {
        "preset".to_string()
    } else {
        slug
    }
}

/// A slug that is free to use, never shadowing a built-in or another custom.
pub(super) fn unique_id(name: &str, file: &PresetFile) -> String {
    let base = slugify(name);
    let taken = |candidate: &str| {
        builtins::all().iter().any(|preset| preset.id == candidate)
            || file.custom.iter().any(|preset| preset.id == candidate)
    };

    if !taken(&base) {
        return base;
    }

    for suffix in 2..10_000 {
        let candidate = format!("{base}-{suffix}");
        if !taken(&candidate) {
            return candidate;
        }
    }

    format!("{base}-{}", std::process::id())
}
