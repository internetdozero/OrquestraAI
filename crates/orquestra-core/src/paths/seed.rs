//! First-launch population of an empty vault from a user-scoped dataset.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{CoreError, CoreResult};

/// Optional one-time dataset used to populate an empty vault on first launch.
///
/// User-scoped by design: `<root>/seed-skills`, overridable with
/// `ORQUESTRA_SEED_SKILLS_DIR` for operators carrying an external dataset.
/// A world-writable default (the old `/mnt/amnesic` path) would let any
/// local user plant skills into someone else's first launch; whatever path
/// resolves here must additionally pass the symlink refusal in
/// [`seed_vault`].
pub fn seed_skills_dir(root: &Path) -> PathBuf {
    std::env::var_os("ORQUESTRA_SEED_SKILLS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("seed-skills"))
}

/// Populate an empty vault from the user-scoped seed dir, copying directory
/// trees verbatim. A missing seed directory, or a non-empty vault, is not
/// an error. A symlinked seed root fails closed: it would otherwise let an
/// attacker redirect first-launch seeding at an arbitrary tree.
pub(super) fn seed_vault(root: &Path, vault: &Path) -> CoreResult<()> {
    let mut entries =
        fs::read_dir(vault).map_err(|source| CoreError::io("read vault", vault, source))?;
    if entries.next().is_some() {
        return Ok(());
    }

    let seed = seed_skills_dir(root);
    if fs::symlink_metadata(&seed)
        .ok()
        .is_some_and(|meta| meta.file_type().is_symlink())
    {
        return Err(CoreError::unsafe_path(
            "seed from",
            &seed,
            "not a real directory",
        ));
    }
    if !seed.is_dir() {
        return Ok(());
    }

    let seed_entries =
        fs::read_dir(&seed).map_err(|source| CoreError::io("read seed dataset", &seed, source))?;

    for entry in seed_entries.flatten() {
        let source = entry.path();
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }

        let destination = vault.join(entry.file_name());
        if destination.exists() {
            continue;
        }

        copy_dir_all(&source, &destination)?;
    }

    Ok(())
}

pub fn copy_dir_all(source: &Path, destination: &Path) -> CoreResult<()> {
    fs::create_dir_all(destination).map_err(|error| {
        CoreError::io_with_context("create", destination, "while seeding", error)
    })?;

    let entries = fs::read_dir(source).map_err(|error| CoreError::io("read", source, error))?;

    for entry in entries {
        let entry = entry.map_err(|error| CoreError::io("read", source, error))?;
        let kind = entry
            .file_type()
            .map_err(|error| CoreError::io("inspect", entry.path(), error))?;
        let target = destination.join(entry.file_name());

        if kind.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target)
                .map_err(|error| CoreError::copy(entry.path(), &target, error))?;
        }
        // Symlinks and special files are intentionally skipped.
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_seed_root(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "orquestra-seed-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("vault")).expect("vault dir");
        root
    }

    /// Task #281: the default seed lives inside the storage root, never on
    /// a world-writable mount.
    #[test]
    fn seed_defaults_to_a_dir_inside_the_storage_root() {
        // The operator override is process-global; only assert the default
        // when the test runner did not set it.
        if std::env::var_os("ORQUESTRA_SEED_SKILLS_DIR").is_none() {
            let root = PathBuf::from("/tmp/orquestra-test-root");
            assert_eq!(seed_skills_dir(&root), root.join("seed-skills"));
        }
    }

    /// Task #281: first launch copies the user-scoped seed into an empty vault.
    #[test]
    fn seed_vault_copies_from_the_user_scoped_seed_dir() {
        let root = temp_seed_root("copy");
        let vault = root.join("vault");
        fs::create_dir_all(root.join("seed-skills/demo")).expect("seed skill");
        fs::write(root.join("seed-skills/demo/SKILL.md"), "demo\n").expect("seed file");
        seed_vault(&root, &vault).expect("seed");
        assert!(vault.join("demo/SKILL.md").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    /// Task #281: a symlinked seed root fails closed instead of seeding the
    /// vault from an attacker-chosen tree.
    #[cfg(unix)]
    #[test]
    fn seed_vault_refuses_a_symlinked_seed_dir() {
        let root = temp_seed_root("link");
        let vault = root.join("vault");
        let outside = root.join("outside");
        fs::create_dir_all(outside.join("evil")).expect("outside");
        fs::write(outside.join("evil/SKILL.md"), "evil\n").expect("evil file");
        std::os::unix::fs::symlink(&outside, root.join("seed-skills")).expect("link");
        assert!(
            seed_vault(&root, &vault).is_err(),
            "a symlinked seed must not plant skills"
        );
        assert!(
            fs::read_dir(&vault).expect("read vault").next().is_none(),
            "the vault must stay empty"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
