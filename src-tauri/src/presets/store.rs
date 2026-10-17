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

/// On-disk shape: the operator's profiles and the applied one.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PresetFile {
    #[serde(default)]
    pub(super) custom: Vec<Preset>,
    #[serde(default)]
    pub(super) active: Option<String>,
}

impl PresetFile {
    #[cfg(test)]
    pub(super) fn custom(&self) -> &[Preset] {
        &self.custom
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
    let (skills, mcps) = capture_live_state()?;
    let id = unique_id(&name, &file);

    let preset = Preset {
        id: id.clone(),
        name,
        description: description.unwrap_or_default().trim().to_string(),
        icon: "custom".to_string(),
        skills,
        mcps,
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

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

pub(super) fn load_current() -> Result<PresetFile, String> {
    load_from(&root()?)
}

/// Built-ins first, then custom profiles, with `active` recomputed from the
/// stored id so the flag can never drift from what was actually applied.
pub(super) fn merged(file: &PresetFile) -> Vec<Preset> {
    let active = file.active();

    let mut presets = builtins::all();
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

/// The skills enabled in any scope and the MCPs enabled anywhere, which is the
/// combination the operator currently sees when they hit "save".
fn capture_live_state() -> Result<(Vec<String>, Vec<String>), String> {
    let mut skill_ids = std::collections::BTreeSet::new();
    let scopes = std::iter::once(None).chain(crate::harness::harness_ids().map(Some));

    for scope in scopes {
        for skill in crate::skills::list_skills(scope.map(str::to_string))? {
            if skill.harness_enabled {
                skill_ids.insert(skill.id);
            }
        }
    }

    let mcps = crate::mcp::list_mcps()?
        .into_iter()
        .filter(|server| server.global_enabled || !server.enabled_harnesses.is_empty())
        .map(|server| server.id)
        .collect();

    Ok((skill_ids.into_iter().collect(), mcps))
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
