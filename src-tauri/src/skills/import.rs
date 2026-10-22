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

/// Clone a git URL shallowly into the OS temp scratch root and import the skill it
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
    // `staging` is `<fresh>/repo`; failures must remove the whole fresh tree.
    let fresh = staging.parent().map(Path::to_path_buf);
    let cleanup = || {
        if let Some(dir) = &fresh {
            let _ = fs::remove_dir_all(dir);
        } else {
            let _ = fs::remove_dir_all(&staging);
        }
    };
    if let Err(error) = clone_shallow(&clone_url, &staging) {
        cleanup();
        return Err(error);
    }

    let source = locate_skill_dir(&staging, subpath.as_deref()).ok_or_else(|| {
        cleanup();
        match subpath {
            Some(anchor) => format!(
                "cloned repo holds no SKILL.md under subpath {anchor:?} — \
                 check the anchor or drop the #fragment for auto-discovery"
            ),
            None => "cloned repo holds no SKILL.md (checked the root, one level down and monorepo layouts)".to_string(),
        }
    })?;

    let imported = import_dir(&source);
    if let Some(dir) = staging.parent() {
        let _ = fs::remove_dir_all(dir);
    } else {
        let _ = fs::remove_dir_all(&staging);
    }
    imported
}

fn staging_dir() -> Result<PathBuf, String> {
    let staging = crate::paths::fresh_scratch_dir("orquestra-git-import")?;
    let repo = staging.join("repo");
    fs::create_dir_all(&repo)
        .map_err(|error| format!("failed to create {}: {error}", repo.display()))?;
    Ok(repo)
}

fn import_dir(source: &Path) -> Result<SkillItem, String> {
    // Before any vault write. `is_dir` follows, and copying a link would
    // read the target with `read_dir`.
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("not a folder: {} ({error})", source.display()))?;
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
        let root = crate::paths::fresh_scratch_dir("orquestra-import-link").expect("scratch");
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
    fn staging_uses_the_secure_runtime_root() {
        // Task #325: the runtime root is XDG-aware, so only assert it is
        // self-consistent and actually stages — the exact parent depends on
        // the ambient `XDG_RUNTIME_DIR`.
        let root = crate::paths::scratch_root();
        assert_eq!(root, crate::paths::scratch_root());
        let staged = staging_dir().expect("staging dir");
        assert!(staged.starts_with(&root));
        // `staging_dir` returns `<fresh>/repo`; remove the whole fresh tree.
        let fresh = staged.parent().expect("fresh parent");
        let _ = fs::remove_dir_all(fresh);
    }

    #[test]
    fn rejects_unsupported_git_urls() {
        assert!(import_skill_from_git(String::new()).is_err());
        assert!(import_skill_from_git("ftp://example.com/repo.git".to_string()).is_err());
        assert!(import_skill_from_git("/tmp/local".to_string()).is_err());
    }
}
