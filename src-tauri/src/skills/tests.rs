#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Unit tests for the vault, the projections and the instruction file.

use super::projection::{batch_from, clear_global_baseline, set_global, set_scope};
use super::vault::{
    categorize, estimate_tokens, list_from, normalize_description, split_frontmatter, validate_id,
    Frontmatter,
};
use super::vault::{delete_skill, read_skill_content, save_skill_content};
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
        let root =
            std::env::temp_dir().join(format!("orquestra-{label}-{}-{unique}", std::process::id()));

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
            assert!(list_from(&paths, GLOBAL_SCOPE)
                .unwrap()
                .iter()
                .all(|skill| skill.id != "linked"));
            assert!(set_global(&paths, "linked", true).is_err());
            assert!(fs::symlink_metadata(paths.global_dir().join("linked")).is_err());
            assert!(outside.join(SKILL_FILE).is_file());
        }

        fs::remove_dir_all(&paths.root).unwrap();
    }

    mod projection_cases;

    #[test]
    fn projected_ids_matches_is_projected() {
        use super::projection::{is_projected, projected_ids};
        let paths = setup_two_skills();
        set_global(&paths, "alpha", true).unwrap();
        set_scope(&paths, "beta", "codex", true).unwrap();

        // Decoys: a plain file and a dangling link are not projections.
        fs::write(paths.projection_dir("codex").join("gamma"), "not a link").unwrap();
        std::os::unix::fs::symlink(
            paths.vault.join("missing"),
            paths.projection_dir("codex").join("delta"),
        )
        .unwrap();

        for scope in std::iter::once(GLOBAL_SCOPE).chain(crate::harness::harness_ids()) {
            let dir = paths.projection_dir(scope);
            let set = projected_ids(&dir, &paths.vault);
            for id in ["alpha", "beta", "gamma", "delta", "missing"] {
                assert_eq!(
                    set.contains(id),
                    is_projected(&dir, &paths.vault, id),
                    "{scope}/{id} diverges"
                );
            }
        }
        // Global authorizes everywhere; beta stays codex-only.
        for scope in crate::harness::harness_ids() {
            let set = projected_ids(&paths.projection_dir(scope), &paths.vault);
            assert!(set.contains("alpha"), "{scope} misses inherited alpha");
        }
        let global = projected_ids(&paths.global_dir(), &paths.vault);
        assert!(global.contains("alpha"));
        assert!(!global.contains("beta"));

        // A dangling link whose target string matches a vault path still
        // counts as nothing, exactly like `is_projected`.
        let dangling = temp_paths("dangling");
        dangling.ensure_layout().unwrap();
        let real = dangling.vault.join("real");
        fs::create_dir_all(&real).unwrap();
        fs::write(
            real.join(SKILL_FILE),
            "---\nname: Real\ndescription: Real skill.\n---\n\nBody.\n",
        )
        .unwrap();
        let scope = dangling.projection_dir("codex");
        fs::create_dir_all(&scope).unwrap();
        std::os::unix::fs::symlink(&real, scope.join("real")).unwrap();
        assert!(is_projected(&scope, &dangling.vault, "real"));
        assert!(projected_ids(&scope, &dangling.vault).contains("real"));
        fs::remove_dir_all(&real).unwrap();
        assert!(!is_projected(&scope, &dangling.vault, "real"));
        assert!(!projected_ids(&scope, &dangling.vault).contains("real"));

        // A hand-made relative link to the right folder counts in both.
        fs::create_dir_all(&real).unwrap();
        fs::write(
            real.join(SKILL_FILE),
            "---\nname: Real\ndescription: Real skill.\n---\n\nBody.\n",
        )
        .unwrap();
        fs::remove_file(scope.join("real")).unwrap();
        std::os::unix::fs::symlink(
            std::path::Path::new("../../skills-library/real"),
            scope.join("real"),
        )
        .unwrap();
        assert!(is_projected(&scope, &dangling.vault, "real"));
        assert!(projected_ids(&scope, &dangling.vault).contains("real"));

        // Legacy aliases resolve to their canonical id in both.
        let canon = "orquestra-modular-start";
        let canon_dir = paths.vault.join(canon);
        fs::create_dir_all(&canon_dir).unwrap();
        fs::write(
            canon_dir.join(SKILL_FILE),
            "---\nname: Modular\ndescription: Modular skill.\n---\n\nBody.\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(&canon_dir, paths.global_dir().join("modular-start")).unwrap();
        let global = projected_ids(&paths.global_dir(), &paths.vault);
        assert!(global.contains(canon));
        assert!(!global.contains("modular-start"));
        assert!(is_projected(&paths.global_dir(), &paths.vault, canon));

        fs::remove_dir_all(&paths.root).unwrap();
        fs::remove_dir_all(&dangling.root).unwrap();
    }

    #[test]
    fn security_cache_tracks_tree_changes() {
        use super::security::{cached_scan, scan_skill_security};
        let root = temp_paths("seccache");
        fs::create_dir_all(&root.root).unwrap();
        let dir = root.root.join("skill");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(SKILL_FILE), "# Demo\n").unwrap();

        assert_eq!(cached_scan(&dir), (false, Vec::<String>::new()));
        // Parity with the uncached walk on a nested tree.
        fs::create_dir_all(dir.join("scripts")).unwrap();
        fs::write(dir.join("scripts/deploy"), "#!/bin/sh\n").unwrap();
        fs::write(dir.join("notes.md"), "# notes\n").unwrap();
        assert_eq!(cached_scan(&dir), scan_skill_security(&dir));
        assert!(cached_scan(&dir).0);

        // An in-place helper edit keeps the verdict (fingerprint refreshes).
        fs::write(dir.join("scripts/deploy"), "#!/bin/sh\necho v2\n").unwrap();
        let (has_scripts, files) = cached_scan(&dir);
        assert!(has_scripts);
        assert_eq!(files, vec!["scripts/deploy".to_string()]);

        // Removing every helper reads clean again.
        fs::remove_file(dir.join("scripts/deploy")).unwrap();
        assert_eq!(cached_scan(&dir), (false, Vec::<String>::new()));

        fs::remove_dir_all(&root.root).unwrap();
    }
}
