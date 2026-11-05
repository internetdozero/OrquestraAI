//! Adoption of harness-owned skill files into the central vault.

use std::fs;
use std::path::Path;

use super::super::status::{skill_folders, Walk};
use orquestra_core::paths::copy_dir_all;

/// Copy every discovered skill into the master vault before its source root is
/// replaced. Harness copies win file-name conflicts; vault-only files remain.
pub(super) fn adopt_missing_skills(
    source: &Path,
    vault: &Path,
    adopted: &mut Vec<String>,
    walk: Walk,
) -> Result<(), String> {
    for (name, path) in skill_folders(source, walk)? {
        let destination = vault.join(&name);
        let is_new = !destination.exists();
        refuse_symlinked_vault_tree(&destination)?;
        copy_dir_all(&path, &destination)?;
        if is_new && !adopted.contains(&name) {
            adopted.push(name);
        }
    }

    Ok(())
}

fn refuse_symlinked_vault_tree(path: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("failed to inspect {}: {error}", path.display())),
    };
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "refusing to adopt into symlinked vault skill {}",
            path.display()
        ));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?
        {
            let entry =
                entry.map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            refuse_symlinked_vault_tree(&entry.path())?;
        }
    }
    Ok(())
}
