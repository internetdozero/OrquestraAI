//! Official MCP servers for the Community & Official Hub.
//!
//! Split from [`crate::official`] to keep registry files small. Entries use
//! unified `orquestra-*` ids with monorepo anchors (`#mcps/<id>`) plus the
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
            "orquestra-idea-vault",
            "Idea Vault",
            "Local SQLite idea vault and inter-agent blackboard MCP server (stdio).",
            "mcps/orquestra-idea-vault",
            "https://github.com/internetdozero/idea-vault-mcp.git",
            "idea-vault",
            "idea-vault",
        ),
        official_mcp(
            "orquestra-session-relay",
            "Session Relay",
            "Session handoff briefs from the stores Codex, Claude, Antigravity, OpenCode, Freebuff and Grok already keep (stdio, no cloud).",
            "mcps/orquestra-session-relay",
            "https://github.com/internetdozero/session-relay-mcp.git",
            "session-relay",
            "session-relay",
        ),
        official_mcp(
            "orquestra-janitor",
            "Janitor",
            "Linux system health, orphan-process sweeper and transient cache cleaner for agentic workspaces (stdio, no network).",
            "mcps/orquestra-janitor",
            "https://github.com/internetdozero/janitor-mcp.git",
            "janitor",
            "janitor",
        ),
        official_mcp(
            "orquestra-core",
            "Orquestra Core",
            "MCP control plane for OrquestraAI: project skills just in time, flip MCP servers, read the master AGENTS.md and check harness links.",
            "mcps/orquestra-core",
            "https://github.com/internetdozero/orquestra-core-mcp.git",
            "orquestra-core",
            "orquestra-core",
        ),
        official_mcp(
            "orquestra-cloudflare",
            "Cloudflare",
            "Cloudflare edge MCP server: Workers, KV, R2 and deployment tooling (stdio).",
            "mcps/orquestra-cloudflare",
            "https://github.com/internetdozero/cloudflare-mcp.git",
            "cloudflare",
            "cloudflare-mcp",
        ),
        official_mcp(
            "orquestra-web-search",
            "Web Search",
            "Exa-backed web search returning cited highlights for AI agents (stdio).",
            "mcps/orquestra-web-search",
            "https://github.com/internetdozero/orquestra-extensions.git",
            "web-search",
            "web-search",
        ),
    ]
}
