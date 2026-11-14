//! Symlink projections under `~/.ownhub/active/<scope>`.
//!
//! A skill is enabled for a scope exactly when that scope's folder holds a
//! valid symlink to the vault entry. Global is authoritative: its links are
//! propagated into every harness folder and cannot be turned off per
//! harness.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use super::vault::{skill_directories, skill_directory};
use super::GLOBAL_SCOPE;
use crate::harness::harness_ids;
use crate::paths::{create_dir, OwnHubPaths};
use ownhub_core::links::{
    create_dir_link, is_dir_link, normalize, read_dir_link, remove_dir_link,
};

pub(super) fn set_global(paths: &OwnHubPaths, id: &str, enabled: bool) -> Result<(), String> {
    let vault_item = skill_directory(paths, id)?;
    let mut failures = Vec::new();

    for scope in std::iter::once(GLOBAL_SCOPE).chain(harness_ids()) {
        let directory = paths.projection_dir(scope);
        let result = if enabled {
            link_skill(&vault_item, &directory, id)
        } else {
            unlink_skill(&directory, id)
        };

        if let Err(error) = result {
            failures.push(format!("{scope}: {error}"));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "global update partially failed: {}",
            failures.join("; ")
        ))
    }
}

/// Clear only the Global baseline link, leaving every harness projection
/// untouched. Preset application uses this instead of `set_global(..., false)`
/// so a skill dropped from a preset stays available in harnesses that
/// project it, while the universal baseline converges on the preset.
pub(super) fn clear_global_baseline(paths: &OwnHubPaths, id: &str) -> Result<(), String> {
    // Rejects unknown ids and traversal attempts, mirroring `set_global`.
    skill_directory(paths, id)?;
    unlink_skill(&paths.global_dir(), id)
}

pub(super) fn set_scope(
    paths: &OwnHubPaths,
    id: &str,
    scope: &str,
    enabled: bool,
) -> Result<(), String> {
    let vault_item = skill_directory(paths, id)?;

    if is_projected(&paths.global_dir(), &paths.vault, id) && !enabled {
        return Err(format!(
            "“{id}” is inherited from Global; disable it in Global first"
        ));
    }

    let directory = paths.projection_dir(scope);
    if enabled {
        link_skill(&vault_item, &directory, id)
    } else {
        unlink_skill(&directory, id)
    }
}

pub(super) fn batch_from(paths: &OwnHubPaths, scope: &str, enabled: bool) -> Result<(), String> {
    let global_dir = paths.global_dir();
    // One `read_dir` for the inherited check instead of two `canonicalize`
    // calls per skill (see `projected_ids`).
    let global_set = projected_ids(&global_dir, &paths.vault);
    let mut failures = Vec::new();

    for (id, _) in skill_directories(&paths.vault)? {
        // Inherited skills cannot be disabled per harness; skipping keeps the
        // batch operation successful while Global stays authoritative.
        if scope != GLOBAL_SCOPE && !enabled && global_set.contains(&id) {
            continue;
        }

        let result = if scope == GLOBAL_SCOPE {
            set_global(paths, &id, enabled)
        } else {
            set_scope(paths, &id, scope, enabled)
        };

        if let Err(error) = result {
            failures.push(format!("{id}: {error}"));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} of the skills could not be updated: {}",
            failures.len(),
            failures.join("; ")
        ))
    }
}

/// Missing scope folders are fine. A symlink is not: `lstat` follows every
/// ancestor, so the joined skill id would be a real directory outside.
fn real_projection_dir(directory: &Path) -> Result<(), String> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(format!(
            "refusing to update {}: not a real directory",
            directory.display()
        )),
        Err(error) => Err(format!(
            "failed to inspect {}: {error}",
            directory.display()
        )),
    }
}

fn ensure_link(vault_item: &Path, projection: &Path) -> Result<(), String> {
    let target = vault_item
        .canonicalize()
        .map_err(|error| format!("failed to resolve {}: {error}", vault_item.display()))?;

    match fs::symlink_metadata(projection) {
        Ok(_) if is_dir_link(projection) => {
            if ownhub_core::links::resolves_to(projection, &target) {
                return Ok(());
            }
            remove_dir_link(projection).map_err(|error| {
                format!(
                    "failed to replace stale link {}: {error}",
                    projection.display()
                )
            })?;
        }
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(projection).map_err(|error| {
                format!(
                    "failed to remove physical directory {}: {error}",
                    projection.display()
                )
            })?;
        }
        Ok(_) => {
            fs::remove_file(projection).map_err(|error| {
                format!("failed to remove file {}: {error}", projection.display())
            })?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect {}: {error}",
                projection.display()
            ));
        }
    }

    if let Some(parent) = projection.parent() {
        create_dir(parent)?;
    }

    create_dir_link(&target, projection).map_err(|error| {
        format!(
            "failed to link {} -> {}: {error}",
            projection.display(),
            target.display()
        )
    })
}

fn link_skill(vault_item: &Path, directory: &Path, id: &str) -> Result<(), String> {
    // `active/<scope>` as a directory symlink makes `<id>` a real folder on
    // the far side, and `remove_dir_all` would delete that outside tree.
    real_projection_dir(directory)?;
    ensure_link(vault_item, &directory.join(id))
}

fn remove_projection_entry(directory: &Path, name: &str) -> Result<(), String> {
    let projection = directory.join(name);

    match fs::symlink_metadata(&projection) {
        Ok(_) if is_dir_link(&projection) => {
            remove_dir_link(&projection)
                .map_err(|error| format!("failed to unlink {}: {error}", projection.display()))?;
        }
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(&projection).map_err(|error| {
                format!(
                    "failed to remove directory {}: {error}",
                    projection.display()
                )
            })?;
        }
        Ok(_) => {
            fs::remove_file(&projection).map_err(|error| {
                format!("failed to remove file {}: {error}", projection.display())
            })?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect {}: {error}",
                projection.display()
            ));
        }
    }

    Ok(())
}

pub(super) fn unlink_skill(directory: &Path, id: &str) -> Result<(), String> {
    real_projection_dir(directory)?;
    remove_projection_entry(directory, id)?;

    for (legacy, canonical) in LEGACY_ALIASES {
        if canonical == id {
            let _ = remove_projection_entry(directory, legacy);
        }
    }

    Ok(())
}

/// Historical skill ids that still resolve to their canonical vault folder.
const LEGACY_ALIASES: [(&str, &str); 4] = [
    ("modular-start", "ownhub-modular-start"),
    ("agent-orchestrator", "ownhub-agent-orchestrator"),
    ("autonomous-audit", "ownhub-autonomous-audit"),
    ("smart-build-router", "ownhub-smart-build-router"),
];

/// Every skill id projected in `projection_dir`, read with a single
/// `read_dir` instead of two `canonicalize` calls per skill.
///
/// A name counts only when it is a symlink whose target is exactly the
/// vault folder for that skill — the same validity rule as
/// [`is_projected`], checked without resolving anything: the vault is
/// canonicalized once, and each entry costs one `lstat` plus one
/// `read_link`. Any entry that does not match that fast path falls back to
/// one `canonicalize`: relative links (never created by [`ensure_link`], but
/// possible by hand) and absolute links spelled through a symlinked root,
/// such as macOS `/var` → `/private/var`.
/// Legacy aliases resolve to their canonical id. A set built this way
/// answers membership exactly like [`is_projected`] for every id the
/// caller looks up, so hot loops (`summary_from`, `list_from`) pay
/// ~12 syscalls per scope instead of ~8000 path resolutions.
pub(super) fn projected_ids(projection_dir: &Path, vault: &Path) -> HashSet<String> {
    let mut projected = HashSet::new();
    let Ok(vault_canon) = fs::canonicalize(vault) else {
        return projected;
    };
    let Ok(entries) = fs::read_dir(projection_dir) else {
        return projected;
    };
    for entry in entries.flatten() {
        // Detection never follows the link: a real directory or file wearing
        // a skill's name is not a projection, and on Windows this also
        // recognises a junction.
        let path = entry.path();
        if !is_dir_link(&path) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let canonical = LEGACY_ALIASES
            .iter()
            .find(|(legacy, _)| *legacy == name)
            .map(|(_, canonical)| *canonical)
            .unwrap_or_else(|| name.as_str());
        // Normalized on both sides: on Windows `canonicalize` returns a
        // `\\?\` verbatim path, which would never equal the plain spelling.
        let expected = normalize(&vault_canon.join(canonical));
        // A dangling link whose target string happens to match must not
        // count: `is_projected` refuses it because the resolution fails.
        if !expected.is_dir() {
            continue;
        }
        let matches = match read_dir_link(&path) {
            // The stored target already spells the canonical vault path.
            Ok(target) if target.is_absolute() && normalize(&target) == expected => true,
            // Hand-made relative link, a platform whose `read_dir_link`
            // cannot report the target, or an absolute link written through a
            // symlinked root: resolve this one entry the slow way.
            _ => fs::canonicalize(&path)
                .map(|resolved| normalize(&resolved) == expected)
                .unwrap_or(false),
        };
        if matches {
            projected.insert(canonical.to_string());
        }
    }
    projected
}

/// A projection is only valid when it is a symlink that resolves to the vault
/// folder for that skill. Dangling links count as disabled.
pub(super) fn is_projected(projection_dir: &Path, vault: &Path, id: &str) -> bool {
    let Ok(vault_item) = fs::canonicalize(vault.join(id)) else {
        return false;
    };

    let check_link = |name: &str| -> bool {
        // `resolves_to` canonicalizes and normalizes both sides, and refuses
        // anything that is not a directory link (symlink or junction).
        ownhub_core::links::resolves_to(&projection_dir.join(name), &vault_item)
    };

    if check_link(id) {
        return true;
    }

    for (legacy, canonical) in LEGACY_ALIASES {
        if canonical == id && check_link(legacy) {
            return true;
        }
    }

    false
}
