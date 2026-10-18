//! What each harness resolves right now: link state for its primary
//! paths, and whether any secondary discovery location still feeds it
//! unmanaged skills.

use std::fs;
use std::path::{Path, PathBuf};

use super::{HarnessStatus, SecondaryStatus};
use crate::harness::{self, SecondaryAction};
use crate::paths::OrquestraPaths;
use crate::skills;

/// How far a skill walk may look, and whether it may enter hidden folders.
#[derive(Clone, Copy)]
pub(super) struct Walk {
    max_depth: usize,
    /// Secondary marketplaces hide their skills under dotted containers (Codex
    /// keeps its curated set in `skills/.curated`), so those walks look inside
    /// them; primary walks treat dotted folders as harness bookkeeping.
    descend_hidden: bool,
}

/// Primary harness roots: shallow, and hidden folders are never skills.
pub(super) const PRIMARY_WALK: Walk = Walk {
    max_depth: 2,
    descend_hidden: false,
};

/// Secondary marketplaces bury skills several levels down
/// (`<plugin>/skills/<skill>/SKILL.md`).
pub(super) const SECONDARY_WALK: Walk = Walk {
    max_depth: 6,
    descend_hidden: true,
};

/// Repository bookkeeping that never contains a skill worth walking.
const VCS_DIRS: [&str; 3] = [".git", ".hg", ".svn"];

pub(super) fn statuses_at(home: &Path, paths: &OrquestraPaths) -> Vec<HarnessStatus> {
    harness::HARNESSES
        .iter()
        .map(|spec| {
            let installed = harness::is_installed(spec, home);
            let projection = paths.projection_dir(spec.id);
            let master = paths.agents_path();

            let skills_paths: Vec<String> = spec
                .skills_links
                .iter()
                .map(|rel| home.join(rel).to_string_lossy().into_owned())
                .collect();
            // Config-only harnesses carry no skills directory, so their link
            // state is satisfied by the config path registered below.
            let skills_linked = spec
                .skills_links
                .iter()
                .all(|rel| is_linked_to(&home.join(rel), &projection));

            let instructions_path = spec
                .instructions_rel
                .map(|rel| home.join(rel).to_string_lossy().into_owned());
            let instructions_linked = spec
                .instructions_rel
                .map(|rel| is_linked_to(&home.join(rel), &master))
                .unwrap_or(true);

            let config_path = spec
                .config_rel
                .map(|rel| home.join(rel).to_string_lossy().into_owned());
            let config_linked = spec
                .config_rel
                .map(|rel| config_registers(&home.join(rel), &projection))
                .unwrap_or(true);

            let supported = !spec.skills_links.is_empty() || spec.config_rel.is_some();

            let secondary_paths: Vec<SecondaryStatus> = spec
                .secondary
                .iter()
                .map(|entry| {
                    let path = home.join(entry.rel);
                    SecondaryStatus {
                        cleared: secondary_cleared(
                            &path,
                            entry.action,
                            &projection,
                            &paths.vault,
                        ),
                        path: path.to_string_lossy().into_owned(),
                        action: entry.action,
                        rationale: entry.rationale.to_string(),
                    }
                })
                .collect();
            let secondary_cleared = secondary_paths.iter().all(|entry| entry.cleared);

            let bridged = installed
                && supported
                && skills_linked
                && instructions_linked
                && config_linked
                && secondary_cleared;

            let kind = if config_path.is_some() && skills_paths.is_empty() {
                "config"
            } else if supported {
                "skills"
            } else {
                "unsupported"
            };
            let target = skills_paths
                .first()
                .or(config_path.as_ref())
                .cloned()
                .unwrap_or_default();

            HarnessStatus {
                id: spec.id.to_string(),
                label: spec.label.to_string(),
                kind: kind.to_string(),
                target,
                skills_paths,
                skills_linked,
                instructions_path,
                instructions_linked,
                config_path,
                config_linked,
                secondary_paths,
                secondary_cleared,
                installed,
                supported,
                bridged,
                note: spec.note.map(str::to_string),
            }
        })
        .collect()
}

/// Collect skill folders under a harness directory. Some harnesses (Hermes)
/// nest skills inside category folders, so a bounded walk is required; the
/// walk stops at any folder that is itself a skill.
pub(super) fn skill_folders(root: &Path, walk: Walk) -> Result<Vec<(String, PathBuf)>, String> {
    let mut found = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];

    while let Some((directory, depth)) = stack.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;

        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();

            // A dotted folder is harness bookkeeping rather than a skill, but
            // it can still contain one, so only the VCS folders are skipped
            // outright on a descending walk.
            if !name.starts_with('.') && path.join(skills::SKILL_FILE).is_file() {
                found.push((name, path));
            } else if depth < walk.max_depth
                && (walk.descend_hidden || !name.starts_with('.'))
                && !VCS_DIRS.contains(&name.as_str())
            {
                stack.push((path, depth + 1));
            }
        }
    }

    Ok(found)
}

/// True when a secondary location can no longer feed the harness unmanaged
/// skills. A missing path counts as cleared: there is no backdoor to close.
fn secondary_cleared(
    source: &Path,
    action: SecondaryAction,
    projection: &Path,
    vault: &Path,
) -> bool {
    if !path_present(source) {
        return true;
    }

    match action {
        SecondaryAction::Project => is_linked_to(source, projection),
        SecondaryAction::Quarantine => {
            if is_link(source) || !source.is_dir() {
                false
            } else if dir_is_empty(source) {
                true
            } else {
                // Transient staging/cache folders without any skills do not leak skills into the harness.
                match skill_folders(source, SECONDARY_WALK) {
                    Ok(folders) => folders.is_empty(),
                    Err(_) => false,
                }
            }
        }
        SecondaryAction::AdoptOnly => all_archived(source, vault),
    }
}

/// Every skill folder under `source` already exists in the vault.
fn all_archived(source: &Path, vault: &Path) -> bool {
    if !source.is_dir() {
        return true;
    }

    match skill_folders(source, SECONDARY_WALK) {
        Ok(folders) => folders
            .iter()
            .all(|(name, _)| vault.join(name).join(skills::SKILL_FILE).is_file()),
        Err(_) => false,
    }
}

/// Resolves without following the last component, so a dangling symlink still
/// counts as present.
pub(super) fn path_present(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

pub(super) fn parent_is_dir(path: &Path) -> bool {
    path.parent().map(Path::is_dir).unwrap_or(false)
}

pub(super) fn dir_is_empty(path: &Path) -> bool {
    fs::read_dir(path)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
}

fn config_registers(config: &Path, projection: &Path) -> bool {
    let Ok(content) = fs::read_to_string(config) else {
        return false;
    };
    let Ok(value) = json5::from_str::<serde_json::Value>(&content) else {
        return false;
    };
    let scope = projection.to_string_lossy();

    value
        .get("skills")
        .and_then(|skills| skills.get("paths"))
        .and_then(|paths| paths.as_array())
        .map(|paths| paths.iter().any(|entry| entry.as_str() == Some(scope.as_ref())))
        .unwrap_or(false)
}

pub(super) fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

pub(super) fn is_linked_to(link: &Path, target: &Path) -> bool {
    if !is_link(link) {
        return false;
    }

    match (fs::canonicalize(link), fs::canonicalize(target)) {
        (Ok(current), Ok(expected)) => current == expected,
        _ => false,
    }
}
