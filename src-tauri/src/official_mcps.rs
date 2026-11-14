//! Official MCP servers for the Community & Official Hub.
//!
//! Split from [`crate::official`] to keep registry files small. Entries use
//! unified `ownhub-*` ids with monorepo anchors (`#mcps/<id>`) plus the
//! individual repo as fallback and the pre-monorepo id for status matching.

use crate::official::{OfficialMcpEntry, MONOREPO};

fn official_mcp(
    id: &str,
    name: &str,
    description: &str,
    subpath: &str,
    fallback_repo: &str,
    legacy_id: &str,
    command: &str,
) -> OfficialMcpEntry {
    OfficialMcpEntry {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        repo_url: Some(format!("{MONOREPO}#{subpath}")),
        command: command.to_string(),
        args: Vec::new(),
        legacy_id: Some(legacy_id.to_string()),
        fallback_repo_url: Some(fallback_repo.to_string()),
    }
}

/// The five official MCP servers, unified naming with legacy fallbacks.
pub(crate) fn official_mcps() -> Vec<OfficialMcpEntry> {
    vec![
        official_mcp(
            "ownhub-idea-vault",
            "Idea Vault",
            "Local SQLite idea vault and inter-agent blackboard MCP server (stdio).",
            "mcps/ownhub-idea-vault",
            "https://github.com/internetdozero/idea-vault-mcp.git",
            "idea-vault",
            "idea-vault",
        ),
        official_mcp(
            "ownhub-session-relay",
            "Session Relay",
            "Session handoff briefs from the stores Codex, Claude, Antigravity, OpenCode and Grok already keep (stdio, no cloud).",
            "mcps/ownhub-session-relay",
            "https://github.com/internetdozero/session-relay-mcp.git",
            "session-relay",
            "session-relay",
        ),
        official_mcp(
            "ownhub-janitor",
            "Janitor",
            "Linux system health, orphan-process sweeper and transient cache cleaner for agentic workspaces (stdio, no network).",
            "mcps/ownhub-janitor",
            "https://github.com/internetdozero/janitor-mcp.git",
            "janitor",
            "janitor",
        ),
        official_mcp(
            "ownhub-core",
            "OwnHub Core",
            "MCP control plane for OwnHub: project skills just in time, flip MCP servers, use stored vault credentials blind, read the master AGENTS.md and check harness links.",
            "mcps/ownhub-core",
            "https://github.com/OwnAnAI/ownhub-core-mcp.git",
            "ownhub-core",
            "ownhub-core",
        ),
        official_mcp(
            "ownhub-cloudflare",
            "Cloudflare",
            "Cloudflare edge MCP server: Workers, KV, R2 and deployment tooling (stdio).",
            "mcps/ownhub-cloudflare",
            "https://github.com/internetdozero/cloudflare-mcp.git",
            "cloudflare",
            "cloudflare-mcp",
        ),
        official_mcp(
            "ownhub-web-search",
            "Web Search",
            "Exa-backed web search returning cited highlights for AI agents (stdio).",
            "mcps/ownhub-web-search",
            "https://github.com/OwnAnAI/ownhub-extensions.git",
            "web-search",
            "web-search",
        ),
    ]
}
