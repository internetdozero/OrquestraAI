//! Link primitives for the fused mirror: replacing one entry with a
//! directory link, and moving an entry (link, directory or file) across
//! filesystems.
//!
//! Split out of `fuse_io` because this is the part Windows forced to change:
//! the projections are NTFS junctions there, so every path that used
//! `std::os::unix::fs::symlink` now goes through `ownhub_core::links`.

use std::fs;
use std::path::Path;

use ownhub_core::harness::{self};
use ownhub_core::links::{
    create_dir_link, create_file_link, is_dir_link, read_dir_link, remove_dir_link,
};
use ownhub_core::paths::{copy_dir_all, rebase, OwnHubPaths};

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

    // A junction cannot describe a file, and on Windows the instruction links
    // are files: use the file primitive there instead.
    let created = if target.is_dir() {
        create_dir_link(target, link)
    } else {
        create_file_link(target, link)
    };
    created.map_err(|error| {
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
        if target.is_dir() {
            create_dir_link(&target, destination)
        } else {
            create_file_link(&target, destination)
        }
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

/// The instruction files fusion linked as *files* rather than symlinks: on
/// Windows that is a hard link, which carries the master's bytes without
/// resolving to its name.
///
/// Collected before the instruction import because `write_atomic` publishes
/// the master through a `rename`: the master gets a new inode while a hard
/// link keeps the previous text. Knowing which files are fusion's own links
/// keeps [`resync_file_links`] away from anything the operator wrote.
pub(crate) fn file_linked_instructions(
    home: &Path,
    paths: &OwnHubPaths,
) -> Vec<std::path::PathBuf> {
    let master = paths.agents_path();
    let mut linked = Vec::new();

    for spec in harness::HARNESSES {
        if !harness::is_installed(spec, home) {
            continue;
        }
        for rel in spec.instructions_rels {
            let source = rebase(home, rel);
            if source.is_file() && !is_link(&source) && is_linked_to(&source, &master) {
                linked.push(source);
            }
        }
    }

    linked
}

/// Re-point the file links collected above at the master the import just
/// published, since a `rename` left them holding the old text.
pub(crate) fn resync_file_links(
    linked: &[std::path::PathBuf],
    paths: &OwnHubPaths,
) -> Vec<String> {
    let master = paths.agents_path();
    let mut errors = Vec::new();

    for source in linked {
        if !source.is_file() || is_linked_to(source, &master) {
            continue;
        }
        let relinked = fs::remove_file(source)
            .map_err(|error| error.to_string())
            .and_then(|()| create_file_link(&master, source).map_err(|error| error.to_string()));
        if let Err(error) = relinked {
            errors.push(format!(
                "failed to refresh instruction link {}: {error}",
                source.display()
            ));
        }
    }

    errors
}
