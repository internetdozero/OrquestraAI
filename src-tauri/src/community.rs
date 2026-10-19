//! Vetted community picks for the Community & Official Hub.
//!
//! Split from [`crate::official`] to keep the registry files small. Entries
//! are prompt-first unless `has_scripts` is set; reserved slots carry
//! `repo_url: None` so the UI shows `unpublished` gracefully.

use crate::official::{tags, OfficialSkillEntry};

fn community_skill(
    id: &str,
    description: &str,
    category: &str,
    repo_url: Option<&str>,
    tag_list: &[&str],
    has_scripts: bool,
) -> OfficialSkillEntry {
    OfficialSkillEntry {
        id: id.to_string(),
        name: id.to_string(),
        description: description.to_string(),
        category: category.to_string(),
        repo_url: repo_url.map(str::to_string),
        official: false,
        tier: "community".to_string(),
        tags: tags(tag_list),
        has_scripts,
        legacy_id: None,
        fallback_repo_url: None,
    }
}

/// Community skills appended to the official registry.
pub(crate) fn community_skills() -> Vec<OfficialSkillEntry> {
    vec![
        community_skill(
            "docker-helper",
            "Community Docker helper: container, compose and image workflows with safe cleanup commands.",
            "Infra",
            Some("https://github.com/internetdozero/docker-helper.git"),
            &["docker", "infra"],
            true,
        ),
        community_skill(
            "git-workflow-booster",
            "Community Git workflow booster: branches, conventional commits and PR hygiene for small teams.",
            "Infra",
            Some("https://github.com/internetdozero/git-workflow-booster.git"),
            &["git", "workflow"],
            false,
        ),
        community_skill(
            "pr-reviewer",
            "Community PR reviewer: checklist-driven code review with testing and security prompts.",
            "Testing",
            Some("https://github.com/internetdozero/pr-reviewer.git"),
            &["review", "testing"],
            false,
        ),
        community_skill(
            "python-debugger",
            "Community Python debugger: traceback triage, pytest loops and REPL-driven fixes.",
            "Testing",
            Some("https://github.com/internetdozero/python-debugger.git"),
            &["python", "debugging"],
            true,
        ),
        community_skill(
            "web-perf-guide",
            "Reserved community slot: web performance playbook (no published repo yet).",
            "Frontend",
            None,
            &["web-perf", "frontend"],
            false,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::official::official_store;

    #[test]
    fn community_hub_has_tags_and_tiers() {
        let picks = community_skills();
        assert!(picks.len() >= 4, "community picks must be present");
        assert!(picks.iter().any(|skill| skill.has_scripts));
        assert!(picks.iter().any(|skill| skill.repo_url.is_none()));
        for skill in &picks {
            assert!(!skill.official);
            assert_eq!(skill.tier, "community");
        }
        let store = official_store();
        let official_count = store.skills.iter().filter(|skill| skill.official).count();
        assert!(official_count >= 4, "official picks must remain");
    }
}
