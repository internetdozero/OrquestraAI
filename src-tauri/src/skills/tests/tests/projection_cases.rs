//! Projection, import and cache tests split out of the skills suite.

use std::fs;

use super::super::super::agents::{read_agents, write_agents};
use super::super::super::projection::{set_global, unlink_skill};
use super::super::super::vault::{list_from, read_skill_content, SKILL_FILE};
use super::super::super::*;
use super::{setup_two_skills, skill_in, temp_paths};

#[test]
fn unlinks_and_replaces_real_directories() {
    let paths = setup_two_skills();
    fs::create_dir_all(paths.global_dir().join("beta")).unwrap();

    assert!(unlink_skill(&paths.global_dir(), "beta").is_ok());
    assert!(!paths.global_dir().join("beta").exists());

    // Test ensure_link / set_global replaces real directory with symlink
    fs::create_dir_all(paths.global_dir().join("beta")).unwrap();
    assert!(set_global(&paths, "beta", true).is_ok());
    assert!(fs::symlink_metadata(paths.global_dir().join("beta"))
        .unwrap()
        .file_type()
        .is_symlink());

    assert!(set_global(&paths, "../alpha", true).is_err());
    assert!(set_skill_state("alpha", "../codex", true).is_err());

    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn monorepo_url_fragments_extract_and_sanitize() {
    use super::subpath::{sanitize_subpath, split_git_url};

    let (repo, sub) = split_git_url(
        "https://github.com/internetdozero/orquestra-extensions.git#skills/orquestra-modular-start",
    );
    assert_eq!(
        repo,
        "https://github.com/internetdozero/orquestra-extensions.git"
    );
    assert_eq!(sub.as_deref(), Some("skills/orquestra-modular-start"));

    let (plain, none) = split_git_url("https://github.com/o/standalone.git");
    assert!(none.is_none());
    assert_eq!(plain, "https://github.com/o/standalone.git");

    assert_eq!(
        sanitize_subpath("mcps/orquestra-janitor").as_deref(),
        Some("mcps/orquestra-janitor")
    );
    for hostile in ["", "../escape", "a/../b", "a//b", ".", "a/./b"] {
        assert!(
            sanitize_subpath(hostile).is_none(),
            "{hostile:?} must be refused"
        );
    }
}

#[test]
fn monorepo_skill_discovery_finds_nested_layouts() {
    use super::subpath::locate_skill_dir;
    use std::path::PathBuf;

    let root = PathBuf::from("/var/tmp/testing")
        .join(format!("orquestra-tests-mono-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    let skill = root.join("skills").join("nested").join("demo");
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join(SKILL_FILE), "# Demo\n").unwrap();

    let found = locate_skill_dir(&root, None).expect("nested skill");
    assert_eq!(found, skill);
    let anchored = locate_skill_dir(&root, Some("skills/nested/demo")).expect("anchor");
    assert_eq!(anchored, skill);
    assert!(locate_skill_dir(&root, Some("skills/missing")).is_none());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn unified_naming_stamps_legacy_and_canonical_ids() {
    use crate::official::{canonical_skill_id, is_official_skill};

    for id in [
        "orquestra",
        "orquestra-modular-start",
        "orquestra-agent-orchestrator",
        "orquestra-smart-build-router",
        "modular-start",
        "agent-orchestrator",
        "lean-screen",
        "ux-best-practices",
    ] {
        assert!(is_official_skill(id), "{id} should be official");
    }
    assert_eq!(
        canonical_skill_id("modular-start"),
        "orquestra-modular-start"
    );
    assert_eq!(canonical_skill_id("orquestra"), "orquestra");
    assert!(!is_official_skill("docker-helper"));
}

#[test]
fn agents_file_defaults_and_roundtrips() {
    let paths = temp_paths("agents");

    let default_content = read_agents(&paths).unwrap();
    assert!(default_content.starts_with("# Global Agent Instructions"));
    assert!(paths.agents_path().is_file());

    write_agents(&paths, "# Custom\n\n- item\n").unwrap();
    assert_eq!(read_agents(&paths).unwrap(), "# Custom\n\n- item\n");

    let leftovers = fs::read_dir(&paths.root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
        .count();
    assert_eq!(leftovers, 0);

    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn vault_cache_serves_stable_results_across_scopes() {
    let paths = setup_two_skills();
    let first = list_from(&paths, "global").expect("list the vault");
    let second = list_from(&paths, "codex").expect("list the vault again");
    assert_eq!(first.len(), 2);
    assert_eq!(
        first
            .iter()
            .map(|skill| skill.id.clone())
            .collect::<Vec<_>>(),
        second
            .iter()
            .map(|skill| skill.id.clone())
            .collect::<Vec<_>>(),
        "the cached parse must match a fresh one"
    );
    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn vault_cache_reflects_skill_edits() {
    let paths = setup_two_skills();
    assert_eq!(
        skill_in(&list_from(&paths, "global").unwrap(), "alpha").name,
        "Alpha"
    );
    // Different length on purpose: the fingerprint keys on mtime+size,
    // so the edit is caught even on a coarse clock.
    fs::write(
        paths.vault.join("alpha").join(SKILL_FILE),
        "---\nname: Alpha Renamed Longer\ndescription: Changed.\n---\n\nBody.\n",
    )
    .unwrap();
    assert_eq!(
        skill_in(&list_from(&paths, "global").unwrap(), "alpha").name,
        "Alpha Renamed Longer"
    );
    fs::remove_dir_all(&paths.root).unwrap();
}

#[cfg(unix)]
#[test]
fn a_symlinked_skill_file_is_not_listed_or_read() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::path::PathBuf::from("/var/tmp/testing").join(format!(
        "orquestra-skill-link-{}-{unique}",
        std::process::id()
    ));
    let paths = crate::paths::OrquestraPaths {
        vault: root.join("skills-library"),
        active: root.join("active"),
        root: root.clone(),
    };
    paths.ensure_layout().expect("layout");
    let outside = root.join("outside.md");
    fs::write(
        &outside,
        "---\nname: Leaked\ndescription: ssh-private-key-material\n---\n\nsecret body\n",
    )
    .expect("outside skill");
    let linked = paths.vault.join("linked");
    fs::create_dir_all(&linked).expect("linked dir");
    std::os::unix::fs::symlink(&outside, linked.join(SKILL_FILE)).expect("skill symlink");
    let real = paths.vault.join("real");
    fs::create_dir_all(&real).expect("real dir");
    fs::write(
        real.join(SKILL_FILE),
        "---\nname: Real\ndescription: stays\n---\n\nbody\n",
    )
    .expect("real skill");

    let listed = list_from(&paths, "global").expect("list");
    assert!(listed.iter().all(|skill| skill.id != "linked"));
    assert!(listed
        .iter()
        .all(|skill| !skill.description.contains("ssh-private")));
    assert!(listed.iter().any(|skill| skill.id == "real"));
    assert!(read_skill_content(&paths, "linked").is_err());
    assert!(outside.is_file(), "the target stays put");

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn vault_security_scan_stays_live() {
    // The security walk is deliberately NOT cached: a helper script added
    // without touching SKILL.md must still flip the badge on next list.
    let paths = setup_two_skills();
    assert!(!skill_in(&list_from(&paths, "global").unwrap(), "alpha").has_scripts);
    let scripts = paths.vault.join("alpha").join("scripts");
    fs::create_dir_all(&scripts).unwrap();
    fs::write(scripts.join("hook.sh"), "#!/bin/sh\n").unwrap();
    let relisted = list_from(&paths, "global").unwrap();
    let alpha = skill_in(&relisted, "alpha");
    assert!(alpha.has_scripts);
    assert_eq!(alpha.script_files, vec!["scripts/hook.sh".to_string()]);
    fs::remove_dir_all(&paths.root).unwrap();
}

#[cfg(unix)]
#[test]
fn read_agents_does_not_follow_a_symlink() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::path::PathBuf::from("/var/tmp/testing").join(format!(
        "orquestra-agents-link-{}-{unique}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let paths = crate::paths::OrquestraPaths {
        vault: root.join("skills-library"),
        active: root.join("active"),
        root: root.clone(),
    };
    paths.ensure_layout().expect("layout");
    let outside = root.join("secret.md");
    fs::write(&outside, "ssh-private\n").expect("outside agents");
    std::os::unix::fs::symlink(&outside, paths.agents_path()).expect("agents symlink");

    let read = read_agents(&paths);
    assert!(
        read.is_err(),
        "linked AGENTS.md must not be loaded: {read:?}"
    );
    assert!(
        !read.unwrap_or_default().contains("ssh-private"),
        "the target contents must not be returned"
    );
    assert_eq!(fs::read_to_string(&outside).unwrap(), "ssh-private\n");
    assert!(fs::symlink_metadata(paths.agents_path())
        .unwrap()
        .file_type()
        .is_symlink());

    fs::remove_dir_all(&root).expect("cleanup");
}

#[test]
fn projection_symlink_is_not_a_directory_to_delete() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::path::PathBuf::from("/var/tmp/testing").join(format!(
        "orquestra-projection-link-{}-{unique}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let paths = crate::paths::OrquestraPaths {
        vault: root.join("skills-library"),
        active: root.join("active"),
        root: root.clone(),
    };
    let skill = paths.vault.join("beta");
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join(SKILL_FILE), "---\nname: Beta\n---\n\nbody\n").unwrap();

    let outside = root.join("outside");
    fs::create_dir_all(outside.join("beta")).unwrap();
    fs::write(outside.join("beta/secret.txt"), "ssh-private\n").unwrap();
    fs::create_dir_all(&paths.active).unwrap();
    std::os::unix::fs::symlink(&outside, paths.global_dir()).unwrap();

    let disabled = set_global(&paths, "beta", false);
    assert!(
        disabled.is_err(),
        "disable followed the projection symlink: {disabled:?}"
    );
    assert_eq!(
        fs::read_to_string(outside.join("beta/secret.txt")).unwrap(),
        "ssh-private\n"
    );

    let enabled = set_global(&paths, "beta", true);
    assert!(
        enabled.is_err(),
        "enable followed the projection symlink: {enabled:?}"
    );
    assert_eq!(
        fs::read_to_string(outside.join("beta/secret.txt")).unwrap(),
        "ssh-private\n"
    );

    fs::remove_dir_all(&root).expect("cleanup");
}
