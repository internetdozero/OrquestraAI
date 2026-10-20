//! Importing external skill folders into the master vault.
//!
//! A skill is a folder holding a `SKILL.md`. Import verifies the source,
//! copies it into `~/.orquestra/skills-library/<folder_name>` with path
//! traversal rejected, and returns the evaluated [`SkillItem`]. Imported
//! skills start unprojected, exactly like seeded vault entries.

use std::fs;
use std::path::{Path, PathBuf};

use super::subpath::{clone_shallow, locate_skill_dir, split_git_url};
use super::vault::{validate_id, SKILL_FILE};
use super::{list_from, SkillItem, GLOBAL_SCOPE};
use crate::paths::{copy_dir_all, open_paths, OrquestraPaths};

/// Copy one skill folder into the vault and return its evaluated item.
pub fn import_skill(source_path: String) -> Result<SkillItem, String> {
    let source = PathBuf::from(&source_path);
    import_dir(&source)
}

/// Clone a git URL shallowly into `/var/tmp/testing` and import the skill it
/// holds. A `#subpath` fragment targets a skill inside a monorepo (e.g.
/// `repo.git#skills/foo`); without one, discovery checks the root, one
/// level down, then monorepo layouts up to 3 levels deep.
pub fn import_skill_from_git(url: String) -> Result<SkillItem, String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("empty git URL".to_string());
    }
    // The `#subpath` anchor is stripped before scheme validation and cloning.
    let (clone_url, subpath) = split_git_url(&url);
    if !(clone_url.starts_with("https://")
        || clone_url.starts_with("http://")
        || clone_url.starts_with("git@"))
    {
        return Err(format!("unsupported git URL: {url:?}"));
    }

    let staging = staging_dir()?;
    if let Err(error) = clone_shallow(&clone_url, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    let source = locate_skill_dir(&staging, subpath.as_deref()).ok_or_else(|| {
        let _ = fs::remove_dir_all(&staging);
        match subpath {
            Some(anchor) => format!(
                "cloned repo holds no SKILL.md under subpath {anchor:?} — \
                 check the anchor or drop the #fragment for auto-discovery"
            ),
            None => "cloned repo holds no SKILL.md (checked the root, one level down and monorepo layouts)".to_string(),
        }
    })?;

    let imported = import_dir(&source);
    let _ = fs::remove_dir_all(&staging);
    imported
}

fn staging_dir() -> Result<PathBuf, String> {
    let staging = staging_root().join(format!("orquestra-git-import-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|error| format!("failed to clear {}: {error}", staging.display()))?;
    }
    fs::create_dir_all(&staging)
        .map_err(|error| format!("failed to create {}: {error}", staging.display()))?;
    Ok(staging.join("repo"))
}

fn import_dir(source: &Path) -> Result<SkillItem, String> {
    // Before any vault write. `is_dir` follows, and copying a link would
    // read the target with `read_dir`.
    let metadata = fs::symlink_metadata(source).map_err(|error| {
        format!("not a folder: {} ({error})", source.display())
    })?;
    if !metadata.file_type().is_dir() {
        return Err(format!(
            "refusing to import {}: not a real directory",
            source.display()
        ));
    }
    let skill_file = source.join(SKILL_FILE);
    match fs::symlink_metadata(&skill_file) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => {
            return Err(format!(
                "refusing to import {}: SKILL.md is not a regular file",
                source.display()
            ));
        }
        Err(_) => {
            return Err(format!(
                "no SKILL.md in {} — pick the skill folder itself",
                source.display()
            ));
        }
    }

    let folder = source
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    validate_id(&folder)?;

    let paths = open_paths()?;
    let destination = paths.vault.join(&folder);
    if let Ok(metadata) = fs::symlink_metadata(&destination) {
        if metadata.file_type().is_symlink() {
            // A symlink in the vault is refused by vault listing and lookup.
            // Replace the link with the imported real directory.
            fs::remove_file(&destination).map_err(|error| {
                format!("failed to clear symlink {}: {error}", destination.display())
            })?;
        } else if destination.exists() {
            return Err(format!("a skill named {folder:?} is already in the vault"));
        }
    }

    copy_dir_all(source, &destination)?;
    finish(&paths, &folder, &destination)
}

/// Staging root for git clones. Always `/var/tmp/testing` (disk-backed);
/// never memory-backed `/tmp`, so large clones cannot pressure RAM.
fn staging_root() -> PathBuf {
    PathBuf::from("/var/tmp/testing")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn import_refuses_a_symlinked_directory() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::path::PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-import-link-{}-{unique}",
            std::process::id()
        ));
        let outside = root.join("outside");
        fs::create_dir_all(&outside).expect("outside");
        fs::write(outside.join("SKILL.md"), "secret-outside\n").expect("skill");
        let link = root.join("linked");
        std::os::unix::fs::symlink(&outside, &link).expect("symlink");

        let error = import_skill(link.to_string_lossy().into_owned()).expect_err("refuse");
        assert!(error.contains("not a real directory"), "{error}");
        assert!(outside.join("SKILL.md").is_file(), "the target stays put");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn staging_stays_on_disk_backed_var_tmp() {
        let root = staging_root();
        assert_eq!(root, PathBuf::from("/var/tmp/testing"));
        assert!(
            !root.starts_with("/tmp"),
            "staging must never use memory-backed /tmp"
        );
        let staged = staging_dir().expect("staging dir");
        assert!(staged.starts_with("/var/tmp/testing"));
        let _ = fs::remove_dir_all(root.join(format!(
            "orquestra-git-import-{}",
            std::process::id()
        )));
    }

    #[test]
    fn rejects_unsupported_git_urls() {
        assert!(import_skill_from_git(String::new()).is_err());
        assert!(import_skill_from_git("ftp://example.com/repo.git".to_string()).is_err());
        assert!(import_skill_from_git("/var/tmp/testing/local".to_string()).is_err());
    }
}
