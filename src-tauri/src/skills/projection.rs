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
        Ok(_) => {
            return Err(format!(
                "refusing to overwrite {}: it exists and is not a symlink",
                projection.display()
            ));
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
    ensure_link(vault_item, &directory.join(id))
}

pub(super) fn unlink_skill(directory: &Path, id: &str) -> Result<(), String> {
    let projection = directory.join(id);

    match fs::symlink_metadata(&projection) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::remove_file(&projection).map_err(|error| {
                format!("failed to unlink {}: {error}", projection.display())
            })?;
        }
        Ok(_) => {
            return Err(format!(
                "refusing to unlink {}: it exists and is not a symlink",
                projection.display()
            ));
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

/// A projection is only valid when it is a symlink that resolves to the vault
/// folder for that skill. Dangling links count as disabled.
pub(super) fn is_projected(projection_dir: &Path, vault: &Path, id: &str) -> bool {
    let projection = projection_dir.join(id);
    let metadata = match fs::symlink_metadata(&projection) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    if !metadata.file_type().is_symlink() {
        return false;
    }

    let (Ok(target), Ok(vault_item)) = (
        fs::canonicalize(&projection),
        fs::canonicalize(vault.join(id)),
    ) else {
        return false;
    };

    target == vault_item
}
