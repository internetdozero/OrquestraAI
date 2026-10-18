//! Importing external skill folders into the master vault.
//!
//! A skill is a folder holding a `SKILL.md`. Import verifies the source,
//! copies it into `~/.orquestra/skills-library/<folder_name>` with path
//! traversal rejected, and returns the evaluated [`SkillItem`]. Imported
//! skills start unprojected, exactly like seeded vault entries.

use std::fs;
use std::path::{Path, PathBuf};

use super::vault::{validate_id, SKILL_FILE};
use super::{list_from, SkillItem, GLOBAL_SCOPE};
use crate::paths::{copy_dir_all, open_paths, OrquestraPaths};

/// Copy one skill folder into the vault and return its evaluated item.
pub fn import_skill(source_path: String) -> Result<SkillItem, String> {
    let source = PathBuf::from(&source_path);
    import_dir(&source)
}

/// Clone a git URL shallowly into `/var/tmp/testing` and import the skill it
/// holds (`SKILL.md` at the repo root or one level down).
pub fn import_skill_from_git(url: String) -> Result<SkillItem, String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("empty git URL".to_string());
    }
    if !(url.starts_with("https://") || url.starts_with("http://") || url.starts_with("git@")) {
        return Err(format!("unsupported git URL: {url:?}"));
    }

    let staging = staging_dir()?;
    let output = std::process::Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg(&url)
        .arg(&staging)
        .output()
        .map_err(|error| {
            let _ = fs::remove_dir_all(&staging);
            format!("git is not available: {error}")
        })?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let _ = fs::remove_dir_all(&staging);
        return Err(if detail.is_empty() {
            format!("git clone failed for {url:?}")
        } else {
            format!("git clone failed: {detail}")
        });
    }

    let source = locate_skill_dir(&staging).ok_or_else(|| {
        let _ = fs::remove_dir_all(&staging);
        "cloned repo holds no SKILL.md (checked the root and one level down)".to_string()
    })?;

    let imported = import_dir(&source);
    let _ = fs::remove_dir_all(&staging);
    imported
}

fn staging_dir() -> Result<PathBuf, String> {
    let staging = PathBuf::from("/var/tmp/testing")
        .join(format!("orquestra-git-import-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|error| format!("failed to clear {}: {error}", staging.display()))?;
    }
    fs::create_dir_all(&staging)
        .map_err(|error| format!("failed to create {}: {error}", staging.display()))?;
    Ok(staging.join("repo"))
}

fn locate_skill_dir(staging: &Path) -> Option<PathBuf> {
    if staging.join(SKILL_FILE).is_file() {
        return Some(staging.to_path_buf());
    }
    let entries = fs::read_dir(staging).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join(SKILL_FILE).is_file() {
            return Some(path);
        }
    }
    None
}

fn import_dir(source: &Path) -> Result<SkillItem, String> {
    if !source.is_dir() {
        return Err(format!("not a folder: {}", source.display()));
    }
    if !source.join(SKILL_FILE).is_file() {
        return Err(format!(
            "no SKILL.md in {} — pick the skill folder itself",
            source.display()
        ));
    }

    let folder = source
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    validate_id(&folder)?;

    let paths = open_paths()?;
    let destination = paths.vault.join(&folder);
    if destination.exists() {
        return Err(format!("a skill named {folder:?} is already in the vault"));
    }

    copy_dir_all(source, &destination)?;
    finish(&paths, &folder, &destination)
}

/// Remove a half-copied destination on failure, then evaluate the new skill.
fn finish(paths: &OrquestraPaths, id: &str, destination: &Path) -> Result<SkillItem, String> {
    match list_from(paths, GLOBAL_SCOPE)
        .into_iter()
        .flatten()
        .find(|skill| skill.id == id)
    {
        Some(skill) => Ok(skill),
        None => {
            let _ = fs::remove_dir_all(destination);
            Err(format!("imported {id:?} but it could not be read back"))
        }
    }
}
