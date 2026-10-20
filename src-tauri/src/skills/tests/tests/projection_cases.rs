//! Projection, import and cache tests split out of the skills suite.

use std::fs;

use super::super::super::agents::{read_agents, write_agents};
use super::super::super::projection::{
    batch_from, clear_global_baseline, set_global, set_scope, unlink_skill,
};
use super::super::super::vault::{
    categorize, delete_skill, estimate_tokens, list_from, normalize_description, read_skill_content,
    save_skill_content, split_frontmatter, validate_id, Frontmatter,
};
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
    assert!(
        fs::symlink_metadata(paths.global_dir().join("beta"))
            .unwrap()
            .file_type()
            .is_symlink()
    );

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

    assert_eq!(sanitize_subpath("mcps/orquestra-janitor").as_deref(), Some("mcps/orquestra-janitor"));
    for hostile in ["", "../escape", "a/../b", "a//b", ".", "a/./b"] {
        assert!(sanitize_subpath(hostile).is_none(), "{hostile:?} must be refused");
    }
}

#[test]
fn monorepo_skill_discovery_finds_nested_layouts() {
    use super::subpath::locate_skill_dir;
    use std::path::PathBuf;

    let root = PathBuf::from("/var/tmp/testing").join(format!(
        "orquestra-tests-mono-{}",
        std::process::id()
    ));
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
    assert_eq!(canonical_skill_id("modular-start"), "orquestra-modular-start");
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
        first.iter().map(|skill| skill.id.clone()).collect::<Vec<_>>(),
        second.iter().map(|skill| skill.id.clone()).collect::<Vec<_>>(),
        "the cached parse must match a fresh one"
    );
    fs::remove_dir_all(&paths.root).unwrap();
}

#[test]
fn vault_cache_reflects_skill_edits() {
    let paths = setup_two_skills();
    assert_eq!(skill_in(&list_from(&paths, "global").unwrap(), "alpha").name, "Alpha");
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