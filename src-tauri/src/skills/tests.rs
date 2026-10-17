//! Unit tests for the vault, the projections and the instruction file.

use super::agents::{read_agents, write_agents};
use super::projection::{batch_from, set_global, set_scope, unlink_skill};
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
    fn refuses_to_unlink_real_directories() {
        let paths = setup_two_skills();
        fs::create_dir_all(paths.global_dir().join("beta")).unwrap();

        assert!(unlink_skill(&paths.global_dir(), "beta").is_err());
        assert!(set_global(&paths, "../alpha", true).is_err());
        assert!(set_skill_state("alpha", "../codex", true).is_err());

        fs::remove_dir_all(&paths.root).unwrap();
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
}
