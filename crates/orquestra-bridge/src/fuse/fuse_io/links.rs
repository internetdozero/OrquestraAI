//! Link primitives for the fused mirror: replacing one entry with a
//! directory link, and moving an entry (link, directory or file) across
//! filesystems.
//!
//! Split out of `fuse_io` because this is the part Windows forced to change:
//! the projections are NTFS junctions there, so every path that used
//! `std::os::unix::fs::symlink` now goes through `orquestra_core::links`.

use std::fs;
use std::path::Path;

use orquestra_core::links::{create_dir_link, is_dir_link, read_dir_link, remove_dir_link};
use orquestra_core::paths::copy_dir_all;

use super::{create_dir_all, is_link};
use crate::status::is_linked_to;

pub(crate) fn ensure_symlink(link: &Path, target: &Path) -> Result<(), String> {
    if is_linked_to(link, target) {
        return Ok(());
    }

    if let Ok(metadata) = fs::symlink_metadata(link) {
        if is_dir_link(link) {
            remove_dir_link(link)
                .map_err(|error| format!("failed to clear {}: {error}", link.display()))?;
        } else if metadata.is_file() {
            fs::remove_file(link)
                .map_err(|error| format!("failed to clear {}: {error}", link.display()))?;
        } else {
            return Err(format!(
                "refusing to replace the directory {} with a link",
                link.display()
            ));
        }
    }

    if let Some(parent) = link.parent() {
        create_dir_all(parent)?;
    }

    create_dir_link(target, link).map_err(|error| {
        format!(
            "failed to link {} -> {}: {error}",
            link.display(),
            target.display()
        )
    })
}

pub(crate) fn move_entry(source: &Path, destination: &Path) -> Result<(), String> {
    if is_link(source) {
        let target = read_dir_link(source)
            .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
        if is_dir_link(destination) {
            let _ = remove_dir_link(destination);
        } else {
            let _ = fs::remove_file(destination);
        }
        create_dir_link(&target, destination)
            .map_err(|error| format!("failed to back up link {}: {error}", source.display()))?;
        remove_dir_link(source)
            .map_err(|error| format!("failed to remove {}: {error}", source.display()))?;
        return Ok(());
    }

    if source.is_dir() {
        copy_dir_all(source, destination)?;
        fs::remove_dir_all(source)
            .map_err(|error| format!("failed to remove {}: {error}", source.display()))?;
        return Ok(());
    }

    fs::copy(source, destination)
        .map_err(|error| format!("failed to copy {}: {error}", source.display()))?;
    fs::remove_file(source)
        .map_err(|error| format!("failed to remove {}: {error}", source.display()))
}
