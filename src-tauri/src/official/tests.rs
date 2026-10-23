use super::*;
use std::collections::BTreeSet;

#[test]
fn skill_rule_matches_the_designated_set() {
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
    for id in [
        "caveman",
        "github",
        "modular",
        "modular-path",
        "orquestra-anything",
        "",
    ] {
        assert!(!is_official_skill(id), "{id} should not be official");
    }
}

#[test]
fn aliases_resolve_forward_and_stamp_official() {
    assert_eq!(
        canonical_skill_id("modular-start"),
        "orquestra-modular-start"
    );
    assert_eq!(
        canonical_skill_id("agent-orchestrator"),
        "orquestra-agent-orchestrator"
    );
    assert_eq!(
        canonical_skill_id("autonomous-audit"),
        "orquestra-autonomous-audit"
    );
    assert_eq!(canonical_skill_id("orquestra"), "orquestra");
    assert_eq!(canonical_skill_id("caveman"), "caveman");
    assert_eq!(canonical_mcp_id("idea-vault"), "orquestra-idea-vault");
    assert_eq!(canonical_mcp_id("janitor"), "orquestra-janitor");
    assert_eq!(canonical_mcp_id("orquestra-core"), "orquestra-core");
    assert_eq!(canonical_mcp_id("github"), "github");
}

#[test]
fn mcp_rule_matches_the_designated_set() {
    for id in [
        "orquestra-idea-vault",
        "orquestra-session-relay",
        "orquestra-janitor",
        "orquestra-core",
        "orquestra-cloudflare",
        "idea-vault",
        "session-relay",
        "janitor",
        "cloudflare",
        "my-orquestra-bridge",
    ] {
        assert!(is_official_mcp(id), "{id} should be official");
    }
    for id in ["github", "papinho", "headroom", ""] {
        assert!(!is_official_mcp(id), "{id} should not be official");
    }
}

#[test]
fn monorepo_anchors_have_https_fallbacks() {
    let store = official_store();
    for skill in store.skills.iter().filter(|skill| skill.official) {
        let Some(url) = skill.repo_url.as_deref() else {
            continue;
        };
        assert!(
            url.starts_with("https://github.com/internetdozero/"),
            "bad remote for {}",
            skill.id
        );
        if let Some((_, anchor)) = url.split_once('#') {
            assert!(!anchor.is_empty() && !anchor.contains(".."));
            assert!(
                anchor.starts_with("skills/"),
                "skill anchor must live under skills/: {}",
                skill.id
            );
        }
        if let Some(fallback) = skill.fallback_repo_url.as_deref() {
            assert!(
                fallback.starts_with("https://github.com/") && !fallback.contains('#'),
                "bad fallback for {}",
                skill.id
            );
        }
    }
    for mcp in &store.mcps {
        let url = mcp.repo_url.as_deref().expect("mcp remote");
        let (_, anchor) = url.split_once('#').expect("mcp anchor");
        assert!(anchor.starts_with("mcps/"), "mcp anchor for {}", mcp.id);
        assert!(mcp.legacy_id.is_some(), "mcp legacy id for {}", mcp.id);
    }
}

#[test]
fn registry_entries_agree_with_the_flag_rules() {
    let store = official_store();
    assert!(!store.skills.is_empty() && !store.mcps.is_empty());
    for skill in &store.skills {
        // Tier flags must agree with the id rule; community picks stay
        // non-official while official picks satisfy it.
        assert_eq!(
            skill.official,
            is_official_skill(&skill.id),
            "registry skill {} flag drifted",
            skill.id
        );
        assert_eq!(
            skill.tier == "official",
            skill.official,
            "registry skill {} tier drifted",
            skill.id
        );
        // Reserved slots carry no repo; published ones stay https.
        if skill.repo_url.is_none() {
            assert_eq!(skill.tier, "community", "only community may be unpublished");
        }
    }
    for mcp in &store.mcps {
        assert!(
            is_official_mcp(&mcp.id),
            "registry MCP {} must satisfy the flag rule",
            mcp.id
        );
    }
}

#[test]
fn registry_has_no_duplicate_ids_and_only_https_remotes() {
    // Skill and MCP ids live in separate namespaces (vault folders vs.
    // catalog keys), so uniqueness is checked per list.
    let store = official_store();
    for (kind, entries) in [
        (
            "skill",
            store
                .skills
                .iter()
                .map(|s| (&s.id, &s.repo_url))
                .collect::<Vec<_>>(),
        ),
        (
            "mcp",
            store
                .mcps
                .iter()
                .map(|m| (&m.id, &m.repo_url))
                .collect::<Vec<_>>(),
        ),
    ] {
        let mut seen = BTreeSet::new();
        for (id, repo_url) in entries {
            assert!(seen.insert(id.clone()), "duplicate {kind} id {id}");
            if let Some(url) = repo_url {
                assert!(
                    url.starts_with("https://github.com/"),
                    "unverified remote for {kind} {id}: {url}"
                );
            }
        }
    }
}

#[test]
fn official_skills_have_expected_scripts_and_legacy_ids() {
    let store = official_store();
    for skill in store.skills.iter().filter(|s| s.official) {
        match skill.id.as_str() {
            "orquestra" => {
                assert!(skill.has_scripts);
                assert_eq!(skill.legacy_id, None);
                assert_eq!(skill.fallback_repo_url, None);
            }
            "orquestra-modular-start" => {
                assert!(skill.has_scripts);
                assert_eq!(skill.legacy_id.as_deref(), Some("modular-start"));
                assert_eq!(skill.fallback_repo_url, None);
            }
            "orquestra-agent-orchestrator" => {
                assert!(skill.has_scripts);
                assert_eq!(skill.legacy_id.as_deref(), Some("agent-orchestrator"));
                assert_eq!(skill.fallback_repo_url, None);
            }
            "orquestra-smart-build-router" => {
                assert!(!skill.has_scripts);
                assert_eq!(skill.legacy_id.as_deref(), Some("smart-build-router"));
                assert_eq!(skill.fallback_repo_url, None);
            }
            "orquestra-autonomous-audit" => {
                assert!(!skill.has_scripts);
                assert_eq!(skill.legacy_id.as_deref(), Some("autonomous-audit"));
                assert_eq!(skill.fallback_repo_url, None);
            }
            "lean-screen" | "ux-best-practices" => {
                assert!(!skill.has_scripts);
                assert_eq!(skill.legacy_id, None);
                assert_eq!(skill.fallback_repo_url, None);
            }
            _ => {}
        }
    }
}
