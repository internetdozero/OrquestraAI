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
pub(crate) fn locate_skill_dir(staging: &Path, subpath: Option<&str>) -> Option<PathBuf> {
    if let Some(anchor) = subpath {
        let target = staging.join(anchor);
        return target.is_dir().then_some(target).filter(|dir| {
            dir.join(SKILL_FILE).is_file()
        });
    }
    if staging.join(SKILL_FILE).is_file() {
        return Some(staging.to_path_buf());
    }
    // Depth 1: standalone repos nesting the skill one folder down.
    if let Some(found) = sorted_child_dirs(staging)
        .into_iter()
        .find(|dir| dir.join(SKILL_FILE).is_file())
    {
        return Some(found);
    }
    // Standard monorepo containers, one skill per child folder.
    for container in MONOREPO_DIRS {
        let root = staging.join(container);
        if !root.is_dir() {
            continue;
        }
        if let Some(found) = sorted_child_dirs(&root)
            .into_iter()
            .find(|dir| dir.join(SKILL_FILE).is_file())
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
                if child.join(SKILL_FILE).is_file() {
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
    if vault_dir.join(".git").is_dir() {
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
    let staging = PathBuf::from("/var/tmp/testing")
        .join(format!("orquestra-sync-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| {
            format!("failed to clear {}: {error}", staging.display())
        })?;
    }
    fs::create_dir_all(&staging).map_err(|error| {
        format!("failed to create {}: {error}", staging.display())
    })?;
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
        fs::remove_dir_all(vault_dir).map_err(|error| {
            format!("failed to clear {}: {error}", vault_dir.display())
        })?;
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
                && !SKIPPED.contains(
                    &entry.file_name().to_string_lossy().to_lowercase().as_str(),
                )
        })
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    dirs
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> PathBuf {
        let root = PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-subpath-{label}-{}",
            std::process::id()
        ));
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }
        fs::create_dir_all(&root).expect("scratch");
        root
    }

    fn skill_at(root: &Path, rel: &str) {
        let dir = root.join(rel);
        fs::create_dir_all(&dir).expect("skill dir");
        fs::write(dir.join(SKILL_FILE), "# Demo\n").expect("skill file");
    }

    #[test]
    fn fragments_split_and_sanitize() {
        let (repo, sub) = split_git_url(
            "https://github.com/internetdozero/orquestra-extensions.git#skills/modular-start",
        );
        assert_eq!(
            repo,
            "https://github.com/internetdozero/orquestra-extensions.git"
        );
        assert_eq!(sub.as_deref(), Some("skills/modular-start"));

        let (plain, none) = split_git_url("https://github.com/o/r.git");
        assert_eq!(plain, "https://github.com/o/r.git");
        assert!(none.is_none());

        for hostile in ["", "#", "../escape", "skills/../escape", "a//b", "a\\b", ".", "skills/./x"] {
            let (_, sub) = split_git_url(&format!("https://github.com/o/r.git#{hostile}"));
            assert!(sub.is_none(), "{hostile:?} must be refused");
        }
    }

    #[test]
    fn anchor_wins_and_never_escapes() {
        let root = scratch("anchor");
        skill_at(&root, "skills/alpha");
        skill_at(&root, "other");
        let found = locate_skill_dir(&root, Some("skills/alpha")).expect("anchor");
        assert_eq!(found, root.join("skills/alpha"));
        assert!(locate_skill_dir(&root, Some("missing")).is_none());
        assert!(locate_skill_dir(&root, Some("../escape")).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discovers_standalone_and_monorepo_layouts() {
        let root = scratch("layouts");
        assert!(locate_skill_dir(&root, None).is_none());

        skill_at(&root, "nested/deep/skill");
        let found = locate_skill_dir(&root, None).expect("deep skill");
        assert_eq!(found, root.join("nested/deep/skill"));

        let mono = scratch("mono");
        skill_at(&mono, "skills/beta");
        skill_at(&mono, "skills/alpha");
        let found = locate_skill_dir(&mono, None).expect("monorepo skill");
        assert_eq!(found, mono.join("skills/alpha"), "sorted first wins");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&mono);
    }

    #[test]
    fn root_still_wins_for_standalone_repos() {
        let root = scratch("root");
        skill_at(&root, ".");
        skill_at(&root, "skills/later");
        let found = locate_skill_dir(&root, None).expect("root skill");
        assert_eq!(found, root);
        let _ = fs::remove_dir_all(&root);
    }
}
