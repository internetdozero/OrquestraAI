//! Official tier: curated OrquestraAI skills and MCP servers.
//!
//! The `official` flag on `SkillItem` and `McpServer` is **computed** from
//! the id rules below, never stored: older catalogs without the key load
//! through `#[serde(default)]` and are stamped on the way out, so the flag
//! is always fresh. This module also owns the Personal Store source: the
//! curated entries plus their install material. Every `repo_url` here was
//! verified with `git ls-remote`; entries without one are reserved official
//! slots with no published repo yet, so the UI shows their status but no
//! install button instead of a dead link.

use serde::Serialize;

/// Official skills maintained by OrquestraAI (verified repos at
/// `github.com/internetdozero`). Exact match only: no prefix rule, so
/// reserved `orquestra-*` names are never stamped official by accident.
const OFFICIAL_SKILL_IDS: &[&str] = &["orquestra", "modular-start", "agent-orchestrator"];

/// MCP servers maintained by OrquestraAI.
const OFFICIAL_MCP_IDS: &[&str] = &[
    "idea-vault",
    "session-relay",
    "janitor",
    "orquestra-core",
    "cloudflare",
];

/// True when a vault skill id belongs to the official tier.
pub(crate) fn is_official_skill(id: &str) -> bool {
    OFFICIAL_SKILL_IDS.contains(&id)
}

/// True when a catalog MCP id belongs to the official tier.
pub(crate) fn is_official_mcp(id: &str) -> bool {
    OFFICIAL_MCP_IDS.contains(&id) || id.contains("orquestra")
}

/// One curated skill in the Community & Official Hub.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialSkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    /// Verified GitHub URL, or `None` for a reserved slot with no repo yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_url: Option<String>,
    /// Badge tier: `true` for OrquestraAI-maintained entries.
    #[serde(default)]
    pub official: bool,
    /// `"official"` or `"community"`.
    #[serde(default)]
    pub tier: String,
    /// Free-form badges (e.g. `docker`, `review`, `python`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Static script hint for the security badge (true = contains helpers).
    #[serde(default)]
    pub has_scripts: bool,
}

fn tags(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn official_skill(
    id: &str,
    description: &str,
    category: &str,
    repo_url: Option<&str>,
    tag_list: &[&str],
) -> OfficialSkillEntry {
    OfficialSkillEntry {
        id: id.to_string(),
        name: id.to_string(),
        description: description.to_string(),
        category: category.to_string(),
        repo_url: repo_url.map(str::to_string),
        official: true,
        tier: "official".to_string(),
        tags: tags(tag_list),
        has_scripts: false,
    }
}

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
    }
}

/// One curated MCP server in the Official Store. `command`/`args` are the
/// console-script entry points from each repo's `pyproject.toml`, used to
/// pre-fill a disabled catalog entry the operator enables after installing
/// the package.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialMcpEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_url: Option<String>,
    pub command: String,
    pub args: Vec<String>,
}

/// The whole Personal Store payload behind `list_official_store`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialStore {
    pub skills: Vec<OfficialSkillEntry>,
    pub mcps: Vec<OfficialMcpEntry>,
}

/// Curated registry: designated official skills plus vetted community picks.
/// Community entries are prompt-first unless `has_scripts` is set; reserved
/// slots carry `repo_url: None` so the UI shows `unpublished` gracefully.
pub fn official_store() -> OfficialStore {
    OfficialStore {
        skills: vec![
            official_skill(
                "orquestra",
                "Operational guide for autonomous AI agents inside the OrquestraAI ecosystem: fleet topology, single-source-of-truth rules and safe control-plane interaction.",
                "AI & Agents",
                Some("https://github.com/internetdozero/orquestra.git"),
                &["agents", "orchestration"],
            ),
            official_skill(
                "modular-start",
                "Start a new project, empty repository or scaffold in any language: initialize git, set up a concise root AGENTS.md with file-size caps and a module map, and commit each coherent slice locally.",
                "Infra",
                Some("https://github.com/internetdozero/modular-start.git"),
                &["scaffold", "infra"],
            ),
            official_skill(
                "agent-orchestrator",
                "Fleet orchestration for OrquestraAI agents: dispatch, supervise and coordinate multi-agent work.",
                "AI & Agents",
                Some("https://github.com/internetdozero/agent-orchestrator.git"),
                &["agents", "fleet"],
            ),
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
        ],
        mcps: vec![
            OfficialMcpEntry {
                id: "idea-vault".to_string(),
                name: "Idea Vault".to_string(),
                description: "Local SQLite idea vault and inter-agent blackboard MCP server (stdio).".to_string(),
                repo_url: Some("https://github.com/internetdozero/idea-vault-mcp.git".to_string()),
                command: "idea-vault".to_string(),
                args: Vec::new(),
            },
            OfficialMcpEntry {
                id: "session-relay".to_string(),
                name: "Session Relay".to_string(),
                description: "Session handoff briefs from the stores Codex, Claude, Antigravity, OpenCode, Freebuff and Grok already keep (stdio, no cloud).".to_string(),
                repo_url: Some("https://github.com/internetdozero/session-relay-mcp.git".to_string()),
                command: "session-relay".to_string(),
                args: Vec::new(),
            },
            OfficialMcpEntry {
                id: "janitor".to_string(),
                name: "Janitor".to_string(),
                description: "Linux system health, orphan-process sweeper and transient cache cleaner for agentic workspaces (stdio, no network).".to_string(),
                repo_url: Some("https://github.com/internetdozero/janitor-mcp.git".to_string()),
                command: "janitor".to_string(),
                args: Vec::new(),
            },
            OfficialMcpEntry {
                id: "orquestra-core".to_string(),
                name: "Orquestra Core".to_string(),
                description: "MCP control plane for OrquestraAI: project skills just in time, flip MCP servers, read the master AGENTS.md and check harness links.".to_string(),
                repo_url: Some("https://github.com/internetdozero/orquestra-core-mcp.git".to_string()),
                command: "orquestra-core".to_string(),
                args: Vec::new(),
            },
            OfficialMcpEntry {
                id: "cloudflare".to_string(),
                name: "Cloudflare".to_string(),
                description: "Cloudflare edge MCP server: Workers, KV, R2 and deployment tooling (stdio).".to_string(),
                repo_url: Some("https://github.com/internetdozero/cloudflare-mcp.git".to_string()),
                command: "cloudflare-mcp".to_string(),
                args: Vec::new(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn skill_rule_matches_the_designated_set() {
        for id in ["orquestra", "modular-start", "agent-orchestrator"] {
            assert!(is_official_skill(id), "{id} should be official");
        }
        for id in [
            "caveman",
            "github",
            "modular",
            "modular-path",
            "orquestra-core",
            "orquestra-orchestrator",
            "orquestra-anything",
            "",
        ] {
            assert!(!is_official_skill(id), "{id} should not be official");
        }
    }

    #[test]
    fn mcp_rule_matches_the_designated_set() {
        for id in [
            "idea-vault",
            "session-relay",
            "janitor",
            "orquestra-core",
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
    fn community_hub_has_tags_and_tiers() {
        let store = official_store();
        let official_count = store.skills.iter().filter(|skill| skill.official).count();
        let community_count = store.skills.iter().filter(|skill| !skill.official).count();
        assert!(official_count >= 3, "official picks must remain");
        assert!(community_count >= 4, "community picks must be present");
        assert!(store.skills.iter().any(|skill| skill.has_scripts));
        assert!(store.skills.iter().any(|skill| skill.repo_url.is_none()));
        for skill in &store.skills {
            assert!(
                skill.tier == "official" || skill.tier == "community",
                "bad tier for {}",
                skill.id
            );
        }
    }

    #[test]
    fn registry_has_no_duplicate_ids_and_only_https_remotes() {
        // Skill and MCP ids live in separate namespaces (vault folders vs.
        // catalog keys), so uniqueness is checked per list.
        let store = official_store();
        for (kind, entries) in [
            ("skill", store.skills.iter().map(|skill| (&skill.id, &skill.repo_url)).collect::<Vec<_>>()),
            ("mcp", store.mcps.iter().map(|mcp| (&mcp.id, &mcp.repo_url)).collect::<Vec<_>>()),
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
}
