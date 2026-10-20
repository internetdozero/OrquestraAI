//! Symlink projections under `~/.orquestra/active/<scope>`.
//!
//! A skill is enabled for a scope exactly when that scope's folder holds a
//! valid symlink to the vault entry. Global is authoritative: its links are
//! propagated into every harness folder and cannot be turned off per
//! harness.

use std::fs;
use std::io;
use std::path::Path;

use super::vault::{skill_directories, skill_directory};
use super::GLOBAL_SCOPE;
use crate::harness::harness_ids;
use crate::paths::{create_dir, OrquestraPaths};

pub(super) fn set_global(paths: &OrquestraPaths, id: &str, enabled: bool) -> Result<(), String> {
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
pub(super) fn clear_global_baseline(paths: &OrquestraPaths, id: &str) -> Result<(), String> {
    // Rejects unknown ids and traversal attempts, mirroring `set_global`.
    skill_directory(paths, id)?;
    unlink_skill(&paths.global_dir(), id)
}

pub(super) fn set_scope(
    paths: &OrquestraPaths,
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

pub(super) fn batch_from(paths: &OrquestraPaths, scope: &str, enabled: bool) -> Result<(), String> {
    let global_dir = paths.global_dir();
    let mut failures = Vec::new();

    for (id, _) in skill_directories(&paths.vault)? {
        // Inherited skills cannot be disabled per harness; skipping keeps the
        // batch operation successful while Global stays authoritative.
        if scope != GLOBAL_SCOPE && !enabled && is_projected(&global_dir, &paths.vault, &id) {
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
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if fs::canonicalize(projection)
                .map(|existing| existing == target)
                .unwrap_or(false)
            {
                return Ok(());
            }
            fs::remove_file(projection).map_err(|error| {
                format!("failed to replace stale link {}: {error}", projection.display())
            })?;
        }
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(projection).map_err(|error| {
                format!("failed to remove physical directory {}: {error}", projection.display())
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

    std::os::unix::fs::symlink(&target, projection).map_err(|error| {
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
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::remove_file(&projection).map_err(|error| {
                format!("failed to unlink {}: {error}", projection.display())
            })?;
        }
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(&projection).map_err(|error| {
                format!("failed to remove directory {}: {error}", projection.display())
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

    for (legacy, canonical) in [
        ("modular-start", "orquestra-modular-start"),
        ("agent-orchestrator", "orquestra-agent-orchestrator"),
        ("autonomous-audit", "orquestra-autonomous-audit"),
        ("smart-build-router", "orquestra-smart-build-router"),
    ] {
        if canonical == id {
            let _ = remove_projection_entry(directory, legacy);
        }
    }

    Ok(())
}

/// A projection is only valid when it is a symlink that resolves to the vault
/// folder for that skill. Dangling links count as disabled.
pub(super) fn is_projected(projection_dir: &Path, vault: &Path, id: &str) -> bool {
    let Ok(vault_item) = fs::canonicalize(vault.join(id)) else {
        return false;
    };

    let check_link = |name: &str| -> bool {
        let projection = projection_dir.join(name);
        let Ok(metadata) = fs::symlink_metadata(&projection) else {
            return false;
        };

        if !metadata.file_type().is_symlink() {
            return false;
        }

        fs::canonicalize(&projection)
            .map(|target| target == vault_item)
            .unwrap_or(false)
    };

    if check_link(id) {
        return true;
    }

    for (legacy, canonical) in [
        ("modular-start", "orquestra-modular-start"),
        ("agent-orchestrator", "orquestra-agent-orchestrator"),
        ("autonomous-audit", "orquestra-autonomous-audit"),
        ("smart-build-router", "orquestra-smart-build-router"),
    ] {
        if canonical == id && check_link(legacy) {
            return true;
        }
    }

    false
}
