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

/// Monorepo consolidating the official suite.
pub(crate) const MONOREPO: &str = "https://github.com/internetdozero/orquestra-extensions.git";

/// Official skills (unified `orquestra-*` naming, served from the monorepo).
const OFFICIAL_SKILL_IDS: &[&str] = &[
    "orquestra",
    "orquestra-modular-start",
    "orquestra-agent-orchestrator",
    "orquestra-smart-build-router",
    "orquestra-autonomous-audit",
    "lean-screen",
    "ux-best-practices",
];

/// Official MCP servers (unified naming).
const OFFICIAL_MCP_IDS: &[&str] = &[
    "orquestra-idea-vault",
    "orquestra-session-relay",
    "orquestra-janitor",
    "orquestra-core",
    "orquestra-cloudflare",
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
        "modular-start" => "orquestra-modular-start",
        "agent-orchestrator" => "orquestra-agent-orchestrator",
        "autonomous-audit" => "orquestra-autonomous-audit",
        _ => id,
    }
}

/// True when a catalog MCP id belongs to the official tier. Legacy
/// pre-monorepo ids resolve through [`canonical_mcp_id`] first; the
/// `orquestra` substring keeps third-party bridges stamped.
pub(crate) fn is_official_mcp(id: &str) -> bool {
    OFFICIAL_MCP_IDS.contains(&canonical_mcp_id(id)) || id.contains("orquestra")
}

/// Legacy catalog id → canonical unified id; anything else passes through.
pub(crate) fn canonical_mcp_id(id: &str) -> &str {
    match id {
        "idea-vault" => "orquestra-idea-vault",
        "session-relay" => "orquestra-session-relay",
        "janitor" => "orquestra-janitor",
        "cloudflare" => "orquestra-cloudflare",
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
        has_scripts: false,
        legacy_id: legacy_id.map(str::to_string),
        fallback_repo_url: fallback_repo_url.map(str::to_string),
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
            "orquestra",
            "Operational guide for autonomous AI agents inside the OrquestraAI ecosystem: fleet topology, single-source-of-truth rules and safe control-plane interaction.",
            "AI & Agents",
            Some("https://github.com/internetdozero/orquestra-extensions.git#skills/orquestra"),
            &["agents", "orchestration"],
            None,
            Some("https://github.com/internetdozero/orquestra.git"),
        ),
        official_skill(
            "orquestra-modular-start",
            "Start a new project, empty repository or scaffold in any language: initialize git, set up a concise root AGENTS.md with file-size caps and a module map, and commit each coherent slice locally.",
            "Infra",
            Some("https://github.com/internetdozero/orquestra-extensions.git#skills/orquestra-modular-start"),
            &["scaffold", "infra"],
            Some("modular-start"),
            Some("https://github.com/internetdozero/modular-start.git"),
        ),
        official_skill(
            "orquestra-agent-orchestrator",
            "Fleet orchestration for OrquestraAI agents: dispatch, supervise and coordinate multi-agent work.",
            "AI & Agents",
            Some("https://github.com/internetdozero/orquestra-extensions.git#skills/orquestra-agent-orchestrator"),
            &["agents", "fleet"],
            Some("agent-orchestrator"),
            Some("https://github.com/internetdozero/agent-orchestrator.git"),
        ),
        official_skill(
            "orquestra-smart-build-router",
            "Route build and task dispatches to the cheapest capable lane: sketches, plans and execution with budget guards.",
            "AI & Agents",
            Some("https://github.com/internetdozero/orquestra-extensions.git#skills/orquestra-smart-build-router"),
            &["agents", "routing"],
            None,
            None,
        ),
        official_skill(
            "orquestra-autonomous-audit",
            "Time-boxed autonomous code audit shift: continuous multi-scope sweeps, atomic 1-issue commits with reproduction tests, formal negative passes, and structured ledger persistence.",
            "AI & Agents",
            Some("https://github.com/internetdozero/orquestra-extensions.git#skills/orquestra-autonomous-audit"),
            &["audit", "security", "testing", "autonomous"],
            Some("autonomous-audit"),
            None,
        ),
        official_skill(
            "lean-screen",
            "Stop the generic admin-dashboard screen on any product: bans obvious subtitles, decorative metric heroes, equal KPI cards, card-in-card, and squeezed center columns.",
            "Frontend",
            Some("https://github.com/internetdozero/orquestra-extensions.git#skills/lean-screen"),
            &["frontend", "ui", "layout"],
            None,
            Some("https://github.com/internetdozero/lean-screen.git"),
        ),
        official_skill(
            "ux-best-practices",
            "UX guidelines and design heuristics for autonomous coding agents: audit checklists, flows, states, and anti-patterns.",
            "Frontend",
            Some("https://github.com/internetdozero/orquestra-extensions.git#skills/ux-best-practices"),
            &["frontend", "ux", "accessibility"],
            None,
            Some("https://github.com/internetdozero/ux-best-practices.git"),
        ),
    ];
    skills.extend(crate::community::community_skills());
    let mcps = crate::official_mcps::official_mcps();
    OfficialStore { skills, mcps }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn skill_rule_matches_the_designated_set() {
        for id in [
            "orquestra", "orquestra-modular-start", "orquestra-agent-orchestrator",
            "orquestra-smart-build-router", "modular-start", "agent-orchestrator",
            "lean-screen", "ux-best-practices",
        ] {
            assert!(is_official_skill(id), "{id} should be official");
        }
        for id in ["caveman", "github", "modular", "modular-path", "orquestra-anything", ""] {
            assert!(!is_official_skill(id), "{id} should not be official");
        }
    }

    #[test]
    fn aliases_resolve_forward_and_stamp_official() {
        assert_eq!(canonical_skill_id("modular-start"), "orquestra-modular-start");
        assert_eq!(canonical_skill_id("agent-orchestrator"), "orquestra-agent-orchestrator");
        assert_eq!(canonical_skill_id("autonomous-audit"), "orquestra-autonomous-audit");
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
            "orquestra-idea-vault", "orquestra-session-relay", "orquestra-janitor",
            "orquestra-core", "orquestra-cloudflare", "idea-vault",
            "session-relay", "janitor", "cloudflare", "my-orquestra-bridge",
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
                assert!(anchor.starts_with("skills/"), "skill anchor must live under skills/: {}", skill.id);
            }
            if let Some(fallback) = skill.fallback_repo_url.as_deref() {
                assert!(fallback.starts_with("https://github.com/") && !fallback.contains('#'), "bad fallback for {}", skill.id);
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
            assert_eq!(skill.official, is_official_skill(&skill.id), "registry skill {} flag drifted", skill.id);
            assert_eq!(skill.tier == "official", skill.official, "registry skill {} tier drifted", skill.id);
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
            ("skill", store.skills.iter().map(|s| (&s.id, &s.repo_url)).collect::<Vec<_>>()),
            ("mcp", store.mcps.iter().map(|m| (&m.id, &m.repo_url)).collect::<Vec<_>>()),
        ] {
            let mut seen = BTreeSet::new();
            for (id, repo_url) in entries {
                assert!(seen.insert(id.clone()), "duplicate {kind} id {id}");
                if let Some(url) = repo_url {
                    assert!(url.starts_with("https://github.com/"), "unverified remote for {kind} {id}: {url}");
                }
            }
        }
    }
}
