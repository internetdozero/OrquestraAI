#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Unit tests for the vault, the projections and the instruction file.

use super::agents::{read_agents, write_agents};
use super::projection::{batch_from, clear_global_baseline, set_global, set_scope, unlink_skill};
use super::vault::{delete_skill, read_skill_content, save_skill_content};
use super::vault::{
    categorize, estimate_tokens, list_from, normalize_description, split_frontmatter,
    validate_id, Frontmatter,
};
use super::*;
use std::fs;

use crate::paths::OrquestraPaths;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_paths(label: &str) -> OrquestraPaths {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "orquestra-{label}-{}-{unique}",
            std::process::id()
        ));

        OrquestraPaths {
            vault: root.join("skills-library"),
            active: root.join("active"),
            root,
        }
    }

    fn setup_two_skills() -> OrquestraPaths {
        let paths = temp_paths("scopes");
        paths.ensure_layout().unwrap();

        for (id, name) in [("alpha", "Alpha"), ("beta", "Beta")] {
            let directory = paths.vault.join(id);
            fs::create_dir_all(&directory).unwrap();
            fs::write(
                directory.join(SKILL_FILE),
                format!("---\nname: {name}\ndescription: {name} skill.\n---\n\n{name} body.\n"),
            )
            .unwrap();
        }

        paths
    }

    fn skill_in<'a>(skills: &'a [SkillItem], id: &str) -> &'a SkillItem {
        skills.iter().find(|skill| skill.id == id).unwrap()
    }

    #[test]
    fn parses_folded_frontmatter() {
        let content = "---\nname: caveman\ndescription: >\n  Ultra-compressed mode.\n  Levels: lite, full.\n---\n\nBody here.\n";
        let (yaml, body) = split_frontmatter(content);
        let metadata: Frontmatter = serde_yaml::from_str(yaml.expect("frontmatter")).unwrap();

        assert_eq!(metadata.name.as_deref(), Some("caveman"));
        assert_eq!(
            normalize_description(metadata.description.as_deref().unwrap()),
            "Ultra-compressed mode. Levels: lite, full."
        );
        assert_eq!(body.trim(), "Body here.");
    }

    #[test]
    fn tolerates_missing_frontmatter() {
        let content = "# Title\n\nPlain markdown.\n";
        let (yaml, body) = split_frontmatter(content);

        assert!(yaml.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn estimates_tokens_from_character_count() {
        let content = "x".repeat(380);
        assert_eq!(estimate_tokens(&content), 100);
    }

    #[test]
    fn categories_prefer_identifier_keywords() {
        assert_eq!(categorize("cloud-penetration-testing", "", ""), "Security");
        assert_eq!(categorize("docker-expert", "", ""), "Infra");
        assert_eq!(categorize("react-best-practices", "", ""), "Frontend");
        assert_eq!(categorize("cloudflare-deploy", "", ""), "Cloud");
        assert_eq!(categorize("asdf-unknown", "", ""), "General");
    }

    #[test]
    fn rejects_path_traversal_ids() {
        assert!(validate_id("../escape").is_err());
        assert!(validate_id("nested/skill").is_err());
        assert!(validate_id("").is_err());
        assert!(validate_id(".hidden").is_err());
        assert!(validate_id("valid-skill").is_ok());
        assert!(validate_scope("global").is_ok());
        assert!(validate_scope("codex").is_ok());
        assert!(validate_scope("../escape").is_err());
    }

    #[test]
    fn global_toggle_propagates_and_locks_harnesses() {
        let paths = setup_two_skills();

        let listed = list_from(&paths, GLOBAL_SCOPE).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].name, "Alpha");
        assert!(listed
            .iter()
            .all(|skill| !skill.global_enabled && !skill.harness_enabled));

        set_global(&paths, "alpha", true).unwrap();
        for scope in std::iter::once(GLOBAL_SCOPE).chain(crate::harness::harness_ids()) {
            let link = paths.projection_dir(scope).join("alpha");
            assert!(
                fs::symlink_metadata(&link)
                    .unwrap()
                    .file_type()
                    .is_symlink(),
                "missing {scope} link"
            );
        }

        let global_alpha = list_from(&paths, GLOBAL_SCOPE).unwrap();
        let global_alpha = skill_in(&global_alpha, "alpha");
        assert!(global_alpha.global_enabled);
        assert!(global_alpha.harness_enabled);
        assert!(!global_alpha.inherited_from_global);
        assert!(global_alpha.tokens > 0);

        let codex_list = list_from(&paths, "codex").unwrap();
        let codex_alpha = skill_in(&codex_list, "alpha");
        assert!(codex_alpha.global_enabled);
        assert!(codex_alpha.harness_enabled);
        assert!(codex_alpha.inherited_from_global);
        assert!(!skill_in(&codex_list, "beta").inherited_from_global);

        // Global ON is authoritative: harness scope cannot disable it.
        assert!(set_scope(&paths, "alpha", "codex", false).is_err());

        // Global OFF removes the baseline and every propagated link.
        set_global(&paths, "alpha", false).unwrap();
        for scope in std::iter::once(GLOBAL_SCOPE).chain(crate::harness::harness_ids()) {
            assert!(fs::symlink_metadata(paths.projection_dir(scope).join("alpha")).is_err());
        }

        // With Global OFF a harness can project independently.
        set_scope(&paths, "alpha", "codex", true).unwrap();
        let codex_list = list_from(&paths, "codex").unwrap();
        let codex_alpha = skill_in(&codex_list, "alpha");
        assert!(!codex_alpha.global_enabled);
        assert!(codex_alpha.harness_enabled);
        assert!(!codex_alpha.inherited_from_global);

        let global_list = list_from(&paths, GLOBAL_SCOPE).unwrap();
        assert!(!skill_in(&global_list, "alpha").harness_enabled);
        let antigravity = list_from(&paths, "antigravity").unwrap();
        assert!(!skill_in(&antigravity, "alpha").harness_enabled);

        let summary = summary_from(&paths).unwrap();
        assert_eq!(summary[0].id, "global");
        assert_eq!(summary[0].active, 0);
        assert_eq!(summary[0].total, 2);
        let codex_summary = summary.iter().find(|row| row.id == "codex").unwrap();
        assert_eq!(codex_summary.active, 1);
        let antigravity_summary = summary.iter().find(|row| row.id == "antigravity").unwrap();
        assert_eq!(antigravity_summary.active, 0);

        // Harness batch operations stay isolated.
        batch_from(&paths, "codex", true).unwrap();
        assert!(list_from(&paths, "codex")
            .unwrap()
            .iter()
            .all(|skill| skill.harness_enabled));
        batch_from(&paths, "codex", false).unwrap();
        assert!(list_from(&paths, "codex")
            .unwrap()
            .iter()
            .all(|skill| !skill.harness_enabled));

        // Global batch propagates every skill, and cleans every projection.
        batch_from(&paths, GLOBAL_SCOPE, true).unwrap();
        let hermes = list_from(&paths, "hermes").unwrap();
        assert!(hermes
            .iter()
            .all(|skill| skill.inherited_from_global && skill.harness_enabled));
        batch_from(&paths, GLOBAL_SCOPE, false).unwrap();
        assert!(list_from(&paths, "hermes")
            .unwrap()
            .iter()
            .all(|skill| !skill.harness_enabled));

        fs::remove_dir_all(&paths.root).unwrap();
    }

    #[test]
    fn clearing_the_global_baseline_preserves_harness_links() {
        let paths = setup_two_skills();

        // Global ON propagates everywhere; codex additionally opts in.
        set_global(&paths, "alpha", true).unwrap();
        set_scope(&paths, "alpha", "codex", true).unwrap();

        // The preset path clears only the baseline link.
        clear_global_baseline(&paths, "alpha").unwrap();

        assert!(fs::symlink_metadata(paths.global_dir().join("alpha")).is_err());
        for harness in crate::harness::harness_ids() {
            assert!(
                fs::symlink_metadata(paths.projection_dir(harness).join("alpha")).is_ok(),
                "missing {harness} link"
            );
        }

        // Read model: baseline off, harness projection still effective.
        let global_list = list_from(&paths, GLOBAL_SCOPE).unwrap();
        let global = skill_in(&global_list, "alpha");
        assert!(!global.global_enabled && !global.harness_enabled);
        let codex_list = list_from(&paths, "codex").unwrap();
        let codex = skill_in(&codex_list, "alpha");
        assert!(!codex.global_enabled);
        assert!(codex.harness_enabled);
        assert!(!codex.inherited_from_global);

        // Unknown ids still error instead of silently passing.
        assert!(clear_global_baseline(&paths, "../escape").is_err());
        assert!(clear_global_baseline(&paths, "missing").is_err());

        fs::remove_dir_all(&paths.root).unwrap();
    }

    #[test]
    fn skill_content_roundtrips_and_rejects_blanks() {
        let paths = setup_two_skills();

        let original = read_skill_content(&paths, "alpha").unwrap();
        assert!(original.contains("Alpha"));
        assert!(read_skill_content(&paths, "../escape").is_err());
        assert!(read_skill_content(&paths, "missing").is_err());

        let updated = "---\nname: Alpha\ndescription: Edited.\n---\n\nNew body.\n";
        save_skill_content(&paths, "alpha", updated).unwrap();
        assert_eq!(read_skill_content(&paths, "alpha").unwrap(), updated);
        // No tmp files leak, and the edited skill still indexes.
        assert!(fs::read_dir(paths.vault.join("alpha"))
            .unwrap()
            .flatten()
            .all(|entry| !entry.file_name().to_string_lossy().contains(".tmp-")));
        let listed = list_from(&paths, GLOBAL_SCOPE).unwrap();
        assert_eq!(skill_in(&listed, "alpha").description, "Edited.");

        assert!(save_skill_content(&paths, "alpha", "   \n").is_err());
        assert!(save_skill_content(&paths, "../escape", updated).is_err());

        fs::remove_dir_all(&paths.root).unwrap();
    }

    #[test]
    fn delete_skill_unlinks_everywhere_and_never_escapes_the_vault() {
        let paths = setup_two_skills();
        set_global(&paths, "alpha", true).unwrap();
        set_scope(&paths, "alpha", "codex", true).unwrap();

        delete_skill(&paths, "alpha").unwrap();
        assert!(!paths.vault.join("alpha").exists());
        for scope in std::iter::once(GLOBAL_SCOPE).chain(crate::harness::harness_ids()) {
            assert!(fs::symlink_metadata(paths.projection_dir(scope).join("alpha")).is_err());
        }
        // The sibling skill is untouched.
        assert!(paths.vault.join("beta").join(SKILL_FILE).is_file());
        assert!(delete_skill(&paths, "alpha").is_err());
        assert!(delete_skill(&paths, "../escape").is_err());

        // A symlink entry is refused: the link target must survive.
        let outside = paths.root.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join(SKILL_FILE), "stale\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, paths.vault.join("linked")).unwrap();
        #[cfg(unix)]
        {
            assert!(delete_skill(&paths, "linked").is_err());
            assert!(outside.join(SKILL_FILE).is_file());
        }

        fs::remove_dir_all(&paths.root).unwrap();
    }

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
}
