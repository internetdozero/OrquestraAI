//! Monorepo subpath anchors and deep skill discovery.
//!
//! A git URL may carry a `#subpath` fragment pointing at the skill folder
//! inside a monorepo, e.g.
//! `https://github.com/internetdozero/orquestra-extensions.git#skills/foo`.
//! The fragment is stripped before `git clone` and resolved as the primary
//! target inside the staging tree. Without a fragment, discovery checks the
//! repo root, one level down (standalone layout), then standard monorepo
//! folders (`skills/`, `packages/`, `extensions/`) up to 3 levels deep.
//!
//! Fragments are sanitized exactly like vault ids: absolute paths, parent
//! traversal and empty segments are refused, so a crafted URL can never
//! escape the staging tree.

use std::fs;
use std::path::{Path, PathBuf};

use super::vault::SKILL_FILE;

/// Standard monorepo containers searched before the general deep walk.
const MONOREPO_DIRS: &[&str] = &["skills", "packages", "extensions"];
/// How deep the general walk descends looking for a `SKILL.md`.
const MAX_DEPTH: usize = 3;
/// Directories never descended into during discovery.
const SKIPPED: &[&str] = &[".git", "target", "node_modules"];

/// Split `url#subpath` into the clone URL and an optional sanitized subpath.
/// A missing, empty or hostile fragment yields `None` (plain-repo behavior).
pub(crate) fn split_git_url(url: &str) -> (String, Option<String>) {
    match url.split_once('#') {
        Some((repo, fragment)) => (repo.to_string(), sanitize_subpath(fragment)),
        None => (url.to_string(), None),
    }
}

/// Accept a relative subpath of plain segments; refuse absolute paths,
/// `.`/`..`, empty segments and backslashes.
pub(crate) fn sanitize_subpath(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let mut clean = Vec::new();
    for segment in trimmed.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.contains('\\')
            || segment.contains('#')
            || segment.contains('\0')
        {
            return None;
        }
        clean.push(segment);
    }
    Some(clean.join("/"))
}

/// Locate the skill folder in a cloned staging tree. An explicit subpath
/// wins; otherwise root, one level down, monorepo folders, then a bounded
/// breadth-first walk (sorted, so monorepos resolve deterministically).
/// A skill folder we can copy without leaving the tree. `is_dir` / `is_file`
/// follow symlinks, and `read_dir` on a link opens the target, so a repo that
/// points `skills/foo` outside the clone would be imported wholesale.
fn is_real_dir(dir: &Path) -> bool {
    fs::symlink_metadata(dir)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

fn has_real_skill_file(dir: &Path) -> bool {
    fs::symlink_metadata(dir.join(SKILL_FILE))
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

pub(crate) fn locate_skill_dir(staging: &Path, subpath: Option<&str>) -> Option<PathBuf> {
    if let Some(anchor) = subpath {
        let target = staging.join(anchor);
        return (is_real_dir(&target) && has_real_skill_file(&target)).then_some(target);
    }
    if is_real_dir(staging) && has_real_skill_file(staging) {
        return Some(staging.to_path_buf());
    }
    // Depth 1: standalone repos nesting the skill one folder down.
    if let Some(found) = sorted_child_dirs(staging)
        .into_iter()
        .find(|dir| has_real_skill_file(dir))
    {
        return Some(found);
    }
    // Standard monorepo containers, one skill per child folder.
    for container in MONOREPO_DIRS {
        let root = staging.join(container);
        if !is_real_dir(&root) {
            continue;
        }
        if let Some(found) = sorted_child_dirs(&root)
            .into_iter()
            .find(|dir| has_real_skill_file(dir))
        {
            return Some(found);
        }
    }
    // Bounded general walk for deeper layouts, up to 3 levels.
    let mut frontier = vec![staging.to_path_buf()];
    for _ in 0..MAX_DEPTH {
        let mut next = Vec::new();
        for dir in frontier {
            for child in sorted_child_dirs(&dir) {
                if has_real_skill_file(&child) {
                    return Some(child);
                }
                next.push(child);
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    None
}

/// Shallow-clone `url` into `destination` (which must not exist yet).
/// The caller strips any `#subpath` fragment first.
pub(crate) fn clone_shallow(url: &str, destination: &Path) -> Result<(), String> {
    let output = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url])
        .arg(destination)
        .output()
        .map_err(|error| format!("git is not available: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if detail.is_empty() {
        format!("git clone failed for {url:?}")
    } else {
        format!("git clone failed: {detail}")
    })
}

/// Refresh a vault folder from its upstream repo: fast `pull --ff-only` when
/// it is a git checkout, otherwise a fresh shallow clone (fragment-aware)
/// whose contents replace the folder. `fallback_url` is tried when the
/// primary (usually the monorepo anchor) fails and the monorepo is not yet
/// published. Projection symlinks point at the folder itself, so they stay
/// live across the refresh.
pub(crate) fn refresh_skill_dir(
    vault_dir: &Path,
    primary_url: &str,
    fallback_url: Option<&str>,
) -> Result<(), String> {
    // `git -C` and `remove_dir_all` follow this path. A vault entry that is a
    // symlink would fast-forward, or delete, the outside tree it points at.
    match fs::symlink_metadata(vault_dir) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => {
            return Err(format!(
                "refusing to refresh {}: not a real directory",
                vault_dir.display()
            ));
        }
        Err(error) => {
            return Err(format!(
                "failed to inspect {}: {error}",
                vault_dir.display()
            ));
        }
    }
    // `is_dir` follows symlinks. A `.git` link would make `git pull` fast-forward
    // that other repository instead of this vault folder.
    let real_checkout = fs::symlink_metadata(vault_dir.join(".git"))
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false);
    if real_checkout {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(vault_dir)
            .args(["pull", "--ff-only"])
            .output()
            .map_err(|error| format!("git is not available: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
    }
    let staging = crate::paths::fresh_scratch_dir("orquestra-sync")?;
    let result = refresh_from_clone(&staging, vault_dir, primary_url, fallback_url);
    let _ = fs::remove_dir_all(&staging);
    result
}

fn refresh_from_clone(
    staging: &Path,
    vault_dir: &Path,
    primary_url: &str,
    fallback_url: Option<&str>,
) -> Result<(), String> {
    let mut attempts = vec![primary_url];
    if let Some(fallback) = fallback_url {
        if fallback != primary_url {
            attempts.push(fallback);
        }
    }
    let mut last_error = String::new();
    for (index, url) in attempts.iter().enumerate() {
        let (clone_url, subpath) = split_git_url(url);
        let clone_dir = staging.join(format!("repo-{index}"));
        if clone_shallow(&clone_url, &clone_dir).is_err() {
            last_error = format!("git clone failed for {clone_url:?}");
            continue;
        }
        let Some(source) = locate_skill_dir(&clone_dir, subpath.as_deref()) else {
            last_error = format!("cloned {url:?} holds no {SKILL_FILE}");
            continue;
        };
        fs::remove_dir_all(vault_dir)
            .map_err(|error| format!("failed to clear {}: {error}", vault_dir.display()))?;
        return crate::paths::copy_dir_all(&source, vault_dir);
    }
    Err(last_error)
}

/// Immediate subdirectories of `dir`, sorted by name, skipping VCS/build
/// folders. Sorting keeps monorepo resolution deterministic.
fn sorted_child_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| {
            entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
                && !SKIPPED.contains(&entry.file_name().to_string_lossy().to_lowercase().as_str())
        })
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    dirs
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[path = "subpath_tests.rs"]
mod tests;
