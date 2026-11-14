//! Official tier: curated OwnHub skills and MCP servers.
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

/// Monorepo consolidating the official suite.
pub(crate) const MONOREPO: &str = "https://github.com/OwnAnAI/ownhub-extensions.git";

/// Official skills (unified `ownhub-*` naming, served from the monorepo).
const OFFICIAL_SKILL_IDS: &[&str] = &[
    "ownhub",
    "ownhub-modular-start",
    "ownhub-agent-orchestrator",
    "ownhub-smart-build-router",
    "ownhub-autonomous-audit",
    "lean-screen",
    "ux-best-practices",
];

/// Official MCP servers (unified naming).
const OFFICIAL_MCP_IDS: &[&str] = &[
    "ownhub-idea-vault",
    "ownhub-session-relay",
    "ownhub-janitor",
    "ownhub-core",
    "ownhub-cloudflare",
    "ownhub-web-search",
];

/// True when a vault skill id belongs to the official tier. Legacy
/// pre-monorepo ids resolve through [`canonical_skill_id`] first, so
/// existing projections keep their tier.
pub(crate) fn is_official_skill(id: &str) -> bool {
    OFFICIAL_SKILL_IDS.contains(&canonical_skill_id(id))
}

/// Legacy vault id → canonical unified id; anything else passes through.
pub(crate) fn canonical_skill_id(id: &str) -> &str {
    match id {
        "modular-start" => "ownhub-modular-start",
        "agent-orchestrator" => "ownhub-agent-orchestrator",
        "autonomous-audit" => "ownhub-autonomous-audit",
        "smart-build-router" => "ownhub-smart-build-router",
        _ => id,
    }
}

/// True when a catalog MCP id belongs to the official tier. Legacy
/// pre-monorepo ids resolve through [`canonical_mcp_id`] first; the
/// `ownhub` substring keeps third-party bridges stamped.
pub(crate) fn is_official_mcp(id: &str) -> bool {
    OFFICIAL_MCP_IDS.contains(&canonical_mcp_id(id)) || id.contains("ownhub")
}

/// Legacy catalog id → canonical unified id; anything else passes through.
pub(crate) fn canonical_mcp_id(id: &str) -> &str {
    match id {
        "idea-vault" => "ownhub-idea-vault",
        "session-relay" => "ownhub-session-relay",
        "janitor" => "ownhub-janitor",
        "cloudflare" => "ownhub-cloudflare",
        "web-search" => "ownhub-web-search",
        _ => id,
    }
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
    /// Badge tier: `true` for OwnHub-maintained entries.
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
    /// Pre-monorepo vault id satisfied by this entry in status matching.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_id: Option<String>,
    /// Individual repo used when the monorepo anchor is not yet published.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_repo_url: Option<String>,
}

pub(crate) fn tags(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn official_skill(
    id: &str,
    description: &str,
    category: &str,
    repo_url: Option<&str>,
    tag_list: &[&str],
    has_scripts: bool,
    legacy_id: Option<&str>,
    fallback_repo_url: Option<&str>,
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
        has_scripts,
        legacy_id: legacy_id.map(str::to_string),
        fallback_repo_url: fallback_repo_url.map(str::to_string),
    }
}

/// One curated MCP server in the Official Store. `command`/`args` are the
/// console-script entry points from each repo's `pyproject.toml`, used to
/// pre-fill a catalog entry that the native adapter installs on activation
/// when its executable is not already present.
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
    /// Pre-monorepo catalog id satisfied by this entry in status matching.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_id: Option<String>,
    /// Individual repo used when the monorepo anchor is not yet published.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_repo_url: Option<String>,
}

/// The whole Personal Store payload behind `list_official_store`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialStore {
    pub skills: Vec<OfficialSkillEntry>,
    pub mcps: Vec<OfficialMcpEntry>,
}

/// Curated registry: unified official suite (monorepo anchors with
/// individual-repo fallbacks) plus vetted community picks.
pub fn official_store() -> OfficialStore {
    let mut skills = vec![
        official_skill(
            "ownhub",
            "Operational guide for autonomous AI agents inside the OwnHub ecosystem: fleet topology, single-source-of-truth rules and safe control-plane interaction.",
            "AI & Agents",
            Some("https://github.com/OwnAnAI/ownhub-extensions.git#skills/ownhub"),
            &["agents", "orchestration"],
            true,
            None,
            None,
        ),
        official_skill(
            "ownhub-modular-start",
            "Start a new project, empty repository or scaffold in any language: initialize git, set up a concise root AGENTS.md with file-size caps and a module map, and commit each coherent slice locally.",
            "Infra",
            Some("https://github.com/OwnAnAI/ownhub-extensions.git#skills/ownhub-modular-start"),
            &["scaffold", "infra"],
            true,
            Some("modular-start"),
            None,
        ),
        official_skill(
            "ownhub-agent-orchestrator",
            "Fleet orchestration for OwnHub agents: dispatch, supervise and coordinate multi-agent work.",
            "AI & Agents",
            Some("https://github.com/OwnAnAI/ownhub-extensions.git#skills/ownhub-agent-orchestrator"),
            &["agents", "fleet"],
            true,
            Some("agent-orchestrator"),
            None,
        ),
        official_skill(
            "ownhub-smart-build-router",
            "Route build and task dispatches to the cheapest capable lane: sketches, plans and execution with budget guards.",
            "AI & Agents",
            Some("https://github.com/OwnAnAI/ownhub-extensions.git#skills/ownhub-smart-build-router"),
            &["agents", "routing"],
            false,
            Some("smart-build-router"),
            None,
        ),
        official_skill(
            "ownhub-autonomous-audit",
            "Time-boxed autonomous code audit shift: continuous multi-scope sweeps, atomic 1-issue commits with reproduction tests, formal negative passes, and structured ledger persistence.",
            "AI & Agents",
            Some("https://github.com/OwnAnAI/ownhub-extensions.git#skills/ownhub-autonomous-audit"),
            &["audit", "security", "testing", "autonomous"],
            false,
            Some("autonomous-audit"),
            None,
        ),
        official_skill(
            "lean-screen",
            "Stop the generic admin-dashboard screen on any product: bans obvious subtitles, decorative metric heroes, equal KPI cards, card-in-card, and squeezed center columns.",
            "Frontend",
            Some("https://github.com/OwnAnAI/ownhub-extensions.git#skills/lean-screen"),
            &["frontend", "ui", "layout"],
            false,
            None,
            None,
        ),
        official_skill(
            "ux-best-practices",
            "UX guidelines and design heuristics for autonomous coding agents: audit checklists, flows, states, and anti-patterns.",
            "Frontend",
            Some("https://github.com/OwnAnAI/ownhub-extensions.git#skills/ux-best-practices"),
            &["frontend", "ux", "accessibility"],
            false,
            None,
            None,
        ),
    ];
    skills.extend(crate::community::community_skills());
    let mcps = crate::official_mcps::official_mcps();
    OfficialStore { skills, mcps }
}

#[cfg(test)]
mod tests;
