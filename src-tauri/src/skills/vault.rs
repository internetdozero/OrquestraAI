//! Reading and indexing the master vault.
//!
//! Every skill is a folder holding a `SKILL.md`; the frontmatter names it
//! and describes it, and the body supplies a fallback description. The
//! projection state is layered on top by [`super::projection`].

use std::fs;
use std::path::{Path, PathBuf};

use std::collections::HashSet;

use super::projection::projected_ids;
use super::security::scan_skill_security;
use super::vault_cache::{cached_content, prune_content_cache};
use super::{SkillItem, GLOBAL_SCOPE};
use crate::paths::OrquestraPaths;

/// Folder name expected at the root of every skill in the vault.
pub(crate) use orquestra_core::skills::SKILL_FILE;

pub(super) fn list_from(paths: &OrquestraPaths, scope: &str) -> Result<Vec<SkillItem>, String> {
    let global_dir = paths.global_dir();
    let scope_dir = paths.projection_dir(scope);
    let is_global = scope == GLOBAL_SCOPE;
    // Projection state for the whole listing: two `read_dir` calls instead
    // of two `canonicalize` pairs per skill (see `projected_ids`).
    let global_set = projected_ids(&global_dir, &paths.vault);
    let scope_set = if is_global {
        HashSet::new()
    } else {
        projected_ids(&scope_dir, &paths.vault)
    };
    let mut skills = Vec::new();
    let mut seen = HashSet::new();

    for (id, directory) in skill_directories(&paths.vault)? {
        let canonical = crate::official::canonical_skill_id(&id);
        if canonical != id && paths.vault.join(canonical).is_dir() {
            continue;
        }

        // File bytes and YAML parse come from the vault cache (validated by
        // SKILL.md mtime+size); projection state and the security walk stay
        // live, because neither may ever serve a stale answer.
        let skill_file = directory.join(SKILL_FILE);
        let Some(cached) = cached_content(&directory, &id, &skill_file) else {
            continue;
        };
        seen.insert(directory.clone());

        let global_enabled = global_set.contains(&id);
        let inherited_from_global = !is_global && global_enabled;
        let harness_enabled = if is_global {
            global_enabled
        } else {
            global_enabled || scope_set.contains(&id)
        };

        let (has_scripts, script_files) = scan_skill_security(&directory);

        skills.push(SkillItem {
            id,
            name: cached.name,
            description: cached.description,
            category: cached.category,
            tokens: cached.tokens,
            global_enabled,
            harness_enabled,
            inherited_from_global,
            official: cached.official,
            has_scripts,
            script_files,
        });
    }

    prune_content_cache(&seen);
    skills.sort_by_key(|skill| skill.name.to_lowercase());
    Ok(skills)
}

/// Read the raw `SKILL.md` of one vault skill, for the in-app editor.
pub(super) fn read_skill_content(paths: &OrquestraPaths, id: &str) -> Result<String, String> {
    let directory = skill_directory(paths, id)?;
    let file = directory.join(SKILL_FILE);
    let metadata = fs::symlink_metadata(&file)
        .map_err(|error| format!("failed to read {}: {error}", file.display()))?;
    if !metadata.file_type().is_file() {
        return Err(format!(
            "refusing to read {}: not a regular file",
            file.display()
        ));
    }
    fs::read_to_string(&file).map_err(|error| format!("failed to read {}: {error}", file.display()))
}

/// Replace a skill's `SKILL.md` atomically (tmp file + rename on the same
/// filesystem). Blank content is refused: an empty definition would silently
/// unlist the skill on the next scan.
pub(super) fn save_skill_content(
    paths: &OrquestraPaths,
    id: &str,
    content: &str,
) -> Result<(), String> {
    if content.trim().is_empty() {
        return Err("skill content must not be empty".to_string());
    }
    let directory = skill_directory(paths, id)?;
    let target = directory.join(SKILL_FILE);
    let temporary = directory.join(format!(
        ".{SKILL_FILE}.tmp-{}",
        crate::paths::unique_tmp_suffix()
    ));

    fs::write(&temporary, content).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to write {}: {error}", temporary.display())
    })?;

    fs::rename(&temporary, &target).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to replace {}: {error}", target.display())
    })
}

/// Remove a skill from the vault: unlink every projection, then delete the
/// folder. A vault entry that is not a real directory (symlink, file) is
/// refused so a malicious or broken entry can never escape the vault.
pub(super) fn delete_skill(paths: &OrquestraPaths, id: &str) -> Result<(), String> {
    let directory = skill_directory(paths, id)?;
    let kind = fs::symlink_metadata(&directory)
        .map_err(|error| format!("failed to inspect {}: {error}", directory.display()))?;
    if !kind.file_type().is_dir() {
        return Err(format!(
            "refusing to delete {}: not a real directory",
            directory.display()
        ));
    }

    super::projection::set_global(paths, id, false)?;
    fs::remove_dir_all(&directory)
        .map_err(|error| format!("failed to remove {}: {error}", directory.display()))
}

pub(super) fn skill_directory(paths: &OrquestraPaths, id: &str) -> Result<PathBuf, String> {
    validate_id(id)?;
    let directory = paths.vault.join(id);
    let metadata = fs::symlink_metadata(&directory).map_err(|_| format!("unknown skill: {id}"))?;
    if !metadata.file_type().is_dir() {
        return Err(format!(
            "refusing to use {id}: vault entry is not a real directory"
        ));
    }
    Ok(directory)
}

pub(super) fn skill_directories(vault: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let entries = fs::read_dir(vault)
        .map_err(|error| format!("failed to read vault {}: {error}", vault.display()))?;

    let mut directories = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        // `is_dir` follows symlinks. A vault entry that points outside would
        // be listed and then projected into every harness.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        // `is_file` follows symlinks. A SKILL.md that points outside the vault
        // would be listed and then read into the editor.
        let skill_file = path.join(SKILL_FILE);
        let Ok(file_meta) = fs::symlink_metadata(&skill_file) else {
            continue;
        };
        if !file_meta.file_type().is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || validate_id(&name).is_err() {
            continue;
        }

        directories.push((name, path));
    }

    directories.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(directories)
}

/// Guard against path traversal and hidden/unusual folder names.
pub(super) fn validate_id(id: &str) -> Result<(), String> {
    let invalid = id.is_empty()
        || id == "."
        || id == ".."
        || id.starts_with('.')
        || id.contains('/')
        || id.contains('\\')
        || id.contains('\0');

    if invalid {
        Err(format!("invalid skill id: {id:?}"))
    } else {
        Ok(())
    }
}

mod document;
pub(super) use document::{
    categorize, estimate_tokens, first_paragraph, normalize_description, split_frontmatter,
    Frontmatter,
};
