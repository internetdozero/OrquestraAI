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

/// One curated skill in the Official Store.
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

/// Curated registry: designated official skills and MCP servers.
pub fn official_store() -> OfficialStore {
    OfficialStore {
        skills: vec![
            OfficialSkillEntry {
                id: "orquestra".to_string(),
                name: "orquestra".to_string(),
                description: "Operational guide for autonomous AI agents inside the OrquestraAI ecosystem: fleet topology, single-source-of-truth rules and safe control-plane interaction.".to_string(),
                category: "AI & Agents".to_string(),
                repo_url: Some("https://github.com/internetdozero/orquestra.git".to_string()),
            },
            OfficialSkillEntry {
                id: "modular-start".to_string(),
                name: "modular-start".to_string(),
                description: "Start a new project, empty repository or scaffold in any language: initialize git, set up a concise root AGENTS.md with file-size caps and a module map, and commit each coherent slice locally.".to_string(),
                category: "Infra".to_string(),
                repo_url: Some("https://github.com/internetdozero/modular-start.git".to_string()),
            },
            OfficialSkillEntry {
                id: "agent-orchestrator".to_string(),
                name: "agent-orchestrator".to_string(),
                description: "Fleet orchestration for OrquestraAI agents: dispatch, supervise and coordinate multi-agent work.".to_string(),
                category: "AI & Agents".to_string(),
                repo_url: Some("https://github.com/internetdozero/agent-orchestrator.git".to_string()),
            },
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
            assert!(
                is_official_skill(&skill.id),
                "registry skill {} must satisfy the flag rule",
                skill.id
            );
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
