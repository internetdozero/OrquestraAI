//! Canonical registry of every AI harness OrquestraAI administers.
//!
//! One entry per harness: its id and label, the directory that proves it
//! is installed, the skill directories and instruction file fusion links,
//! the JSONC config that receives `skills.paths`, the MCP envelope it
//! expects, and the secondary discovery locations that bypass its
//! projection. `skills`, `mcp` and `bridge` all read this table, so a
//! harness is described in exactly one place.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// Reserved scope id for the universal baseline projection.
pub const GLOBAL_SCOPE: &str = "global";

/// How fusion handles a secondary discovery location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SecondaryAction {
    /// Replace the folder with a symlink to the harness projection, so the
    /// harness resolves skills only through what the operator enabled.
    Project,
    /// Move every entry into the run backup and leave an empty folder behind.
    /// Reversible, and the folder keeps existing for tools that expect it.
    Quarantine,
    /// Archive the skills into the vault but leave the vendor folder alone,
    /// because the harness ships and needs those built-ins.
    AdoptOnly,
}

/// One secondary discovery location a harness reads beside its projection.
#[derive(Clone, Copy)]
pub struct SecondarySpec {
    /// Path relative to `$HOME`.
    pub rel: &'static str,
    pub action: SecondaryAction,
    /// Why the location is a backdoor, shown in the bridge modal and reports.
    pub rationale: &'static str,
}

/// The envelope a harness expects around the same MCP server definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpFormat {
    /// `{"mcpServers": { "<id>": { command, args, env } }}` — Antigravity,
    /// Copilot and the root of Claude's `~/.claude.json`.
    JsonMcpServers,
    /// `{"mcp": { "<id>": { type, command: [..], enabled, environment } }}` —
    /// OpenCode and Kilo Code.
    JsoncMcpBlock,
    /// `[mcp_servers.<id>]` tables with `[mcp_servers.<id>.env]` — Codex and
    /// xAI Grok (`~/.grok/config.toml`).
    CodexToml,
}

/// Where one harness takes MCP servers, and in which shape.
#[derive(Debug, Clone, Copy)]
pub struct McpSpec {
    /// Config path relative to `$HOME`.
    pub rel: &'static str,
    pub format: McpFormat,
    /// Extra per-entry key some harnesses require (`type = "stdio"`).
    pub entry_type: Option<&'static str>,
}

/// One harness and every path OrquestraAI claims for it.
pub struct HarnessSpec {
    pub id: &'static str,
    pub label: &'static str,
    /// Environment variable that overrides the install directory (e.g.
    /// `CODEX_HOME`). When set and non-empty, it wins over `install_rel`.
    pub env_override: Option<&'static str>,
    /// Directory that must exist for the harness to count as installed.
    pub install_rel: &'static str,
    /// Source paths that all become symlinks to `active/<id>`.
    pub skills_links: &'static [&'static str],
    /// Harness instruction file replaced by a symlink to the master AGENTS.md.
    pub instructions_rel: Option<&'static str>,
    /// JSONC config that receives `skills.paths`.
    pub config_rel: Option<&'static str>,
    /// MCP config this harness reads, when it has an adapter.
    pub mcp: Option<McpSpec>,
    /// Secondary discovery locations that bypass the harness projection. A
    /// harness that finds `SKILL.md` files here stays bloated no matter what
    /// the operator toggles off, so fusion has to claim them too.
    pub secondary: &'static [SecondarySpec],
    pub note: Option<&'static str>,
}

/// The `skills` CLI installs into a shared user root that Codex and Grok both
/// inject into their session context, so both harnesses have to claim it.
const SHARED_AGENT_SKILLS: SecondarySpec = SecondarySpec {
    rel: ".agents/skills",
    action: SecondaryAction::Quarantine,
    rationale: "Shared user root that Codex and Grok inject into every session",
};

pub const HARNESSES: &[HarnessSpec] = &[
    HarnessSpec {
        id: "antigravity",
        label: "Antigravity",
        env_override: Some("ANTIGRAVITY_CONFIG_DIR"),
        install_rel: ".gemini/config",
        skills_links: &[".gemini/config/skills"],
        instructions_rel: Some(".gemini/config/AGENTS.md"),
        config_rel: None,
        mcp: Some(McpSpec {
            rel: ".gemini/config/mcp_config.json",
            format: McpFormat::JsonMcpServers,
            entry_type: None,
        }),
        secondary: &[SecondarySpec {
            rel: ".gemini/antigravity/builtin/skills",
            action: SecondaryAction::AdoptOnly,
            rationale: "Vendor built-ins ship with the app; archived in the vault so \
                        ~/.gemini/config/skills stays the only user source",
        }],
        note: None,
    },
    HarnessSpec {
        id: "codex",
        label: "Codex",
        env_override: Some("CODEX_HOME"),
        install_rel: ".codex",
        skills_links: &[".codex/skills"],
        instructions_rel: Some(".codex/AGENTS.md"),
        config_rel: None,
        mcp: Some(McpSpec {
            rel: ".codex/config.toml",
            format: McpFormat::CodexToml,
            entry_type: None,
        }),
        secondary: &[
            SecondarySpec {
                rel: ".codex/vendor_imports/skills",
                action: SecondaryAction::Quarantine,
                rationale: "Curated vendor checkout Codex indexes beside ~/.codex/skills",
            },
            SecondarySpec {
                rel: ".codex/.tmp/bundled-marketplaces",
                action: SecondaryAction::Quarantine,
                rationale: "Bundled marketplace cache of plugin skills",
            },
            SecondarySpec {
                rel: ".codex/.tmp/marketplaces",
                action: SecondaryAction::Quarantine,
                rationale: "Stale marketplace cache of plugin skills",
            },
            SHARED_AGENT_SKILLS,
        ],
        note: Some("Internal .system folders are moved into the projection first"),
    },
    HarnessSpec {
        id: "claude",
        label: "Claude Code",
        env_override: Some("CLAUDE_CONFIG_DIR"),
        install_rel: ".claude",
        skills_links: &[".claude/skills"],
        instructions_rel: Some(".claude/CLAUDE.md"),
        config_rel: None,
        mcp: Some(McpSpec {
            rel: ".claude.json",
            format: McpFormat::JsonMcpServers,
            entry_type: None,
        }),
        secondary: &[SecondarySpec {
            rel: ".claude/plugins/marketplaces",
            action: SecondaryAction::Quarantine,
            rationale: "Plugin marketplace skills load beside ~/.claude/skills",
        }],
        note: None,
    },
    HarnessSpec {
        id: "freebuff",
        label: "Freebuff",
        env_override: None,
        install_rel: ".freebuff",
        skills_links: &[".freebuff/skills", ".config/manicode/skills"],
        instructions_rel: Some(".config/manicode/AGENTS.md"),
        config_rel: None,
        mcp: None,
        secondary: &[],
        note: Some("Manicode shares the Freebuff projection"),
    },
    HarnessSpec {
        id: "opencode",
        label: "OpenCode",
        env_override: Some("OPENCODE_CONFIG_DIR"),
        install_rel: ".config/opencode",
        skills_links: &[],
        instructions_rel: None,
        config_rel: Some(".config/opencode/opencode.jsonc"),
        mcp: Some(McpSpec {
            rel: ".config/opencode/opencode.jsonc",
            format: McpFormat::JsoncMcpBlock,
            entry_type: None,
        }),
        secondary: &[],
        note: None,
    },
    HarnessSpec {
        id: "kilo",
        label: "Kilo Code",
        env_override: Some("KILO_CONFIG_DIR"),
        install_rel: ".config/kilo",
        skills_links: &[],
        instructions_rel: None,
        config_rel: Some(".config/kilo/kilo.jsonc"),
        mcp: Some(McpSpec {
            rel: ".config/kilo/kilo.jsonc",
            format: McpFormat::JsoncMcpBlock,
            entry_type: None,
        }),
        secondary: &[],
        note: None,
    },
    HarnessSpec {
        id: "copilot",
        label: "Copilot CLI",
        env_override: None,
        install_rel: ".copilot",
        skills_links: &[".copilot/skills"],
        instructions_rel: None,
        config_rel: None,
        mcp: Some(McpSpec {
            rel: ".copilot/mcp-config.json",
            format: McpFormat::JsonMcpServers,
            entry_type: Some("stdio"),
        }),
        secondary: &[],
        note: None,
    },
    HarnessSpec {
        id: "hermes",
        label: "Hermes Agent",
        env_override: None,
        install_rel: ".hermes",
        skills_links: &[".hermes/skills"],
        instructions_rel: None,
        config_rel: None,
        mcp: None,
        secondary: &[SecondarySpec {
            rel: ".hermes/hermes-agent/optional-skills",
            action: SecondaryAction::Quarantine,
            rationale: "Optional skill repository Hermes auto-loads beside ~/.hermes/skills",
        }],
        note: Some("Curator state files are kept inside the projection"),
    },
    HarnessSpec {
        id: "grok",
        label: "xAI Grok",
        env_override: None,
        install_rel: ".grok",
        skills_links: &[".grok/skills"],
        instructions_rel: None,
        config_rel: None,
        mcp: Some(McpSpec {
            rel: ".grok/config.toml",
            format: McpFormat::CodexToml,
            entry_type: None,
        }),
        secondary: &[
            SecondarySpec {
                rel: ".grok/bundled/skills",
                action: SecondaryAction::Project,
                rationale: "Grok resolves its built-ins here and ignores ~/.grok/skills",
            },
            SecondarySpec {
                rel: ".grok/installed-plugins",
                action: SecondaryAction::Quarantine,
                rationale: "Third-party plugin bundles Grok loads on startup",
            },
            SHARED_AGENT_SKILLS,
        ],
        note: None,
    },
];

/// Every harness id, in registry order.
pub fn harness_ids() -> impl Iterator<Item = &'static str> {
    HARNESSES.iter().map(|harness| harness.id)
}

/// Look up one harness by id.
pub fn find(id: &str) -> Option<&'static HarnessSpec> {
    HARNESSES.iter().find(|harness| harness.id == id)
}

/// Where a harness is installed. A set, non-empty `env_override` wins;
/// otherwise the standard home-relative default applies.
pub fn resolve_install_dir(spec: &HarnessSpec, home: &Path) -> PathBuf {
    if let Some(key) = spec.env_override {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                return PathBuf::from(value);
            }
        }
    }
    home.join(spec.install_rel)
}

/// True when the harness's install directory exists.
pub fn is_installed(spec: &HarnessSpec, home: &Path) -> bool {
    resolve_install_dir(spec, home).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// `CODEX_HOME` wins over `~/.codex` while set; removing it restores
    /// the home-relative default, and `is_installed` follows the resolved
    /// path. One test covers every `CODEX_HOME` mutation so parallel
    /// runners cannot interleave a set/remove on the same variable.
    #[test]
    fn env_override_wins_and_falls_back() {
        let spec = find("codex").expect("codex is registered");
        assert_eq!(spec.env_override, Some("CODEX_HOME"));

        let home = std::env::temp_dir().join(format!(
            "orquestra-harness-{}",
            std::process::id()
        ));
        let previous = env::var("CODEX_HOME").ok();

        unsafe { env::set_var("CODEX_HOME", "/tmp/orquestra-codex-mock") };
        assert_eq!(
            resolve_install_dir(spec, &home),
            PathBuf::from("/tmp/orquestra-codex-mock")
        );

        unsafe { env::remove_var("CODEX_HOME") };
        assert_eq!(resolve_install_dir(spec, &home), home.join(".codex"));
        assert!(!is_installed(spec, &home));

        std::fs::create_dir_all(home.join(".codex")).unwrap();
        assert!(is_installed(spec, &home));
        std::fs::remove_dir_all(&home).unwrap();
        assert!(!is_installed(spec, &home));

        if let Some(value) = previous {
            unsafe { env::set_var("CODEX_HOME", value) };
        }
    }

    #[test]
    fn empty_override_falls_back_to_home() {
        let spec = find("claude").expect("claude is registered");
        assert_eq!(spec.env_override, Some("CLAUDE_CONFIG_DIR"));
        let home = Path::new("/tmp/orquestra-home-that-cannot-exist");

        let previous = env::var("CLAUDE_CONFIG_DIR").ok();
        unsafe { env::set_var("CLAUDE_CONFIG_DIR", "   ") };
        assert_eq!(resolve_install_dir(spec, home), home.join(".claude"));

        if let Some(value) = previous {
            unsafe { env::set_var("CLAUDE_CONFIG_DIR", value) };
        } else {
            unsafe { env::remove_var("CLAUDE_CONFIG_DIR") };
        }
    }

    #[test]
    fn harnesses_without_override_use_home() {
        let spec = find("freebuff").expect("freebuff is registered");
        assert_eq!(spec.env_override, None);
        let home = Path::new("/tmp/orquestra-home-that-cannot-exist");
        assert_eq!(resolve_install_dir(spec, home), home.join(".freebuff"));
    }

    #[test]
    fn override_mapping_covers_the_announced_harnesses() {
        assert_eq!(
            find("antigravity").expect("registered").env_override,
            Some("ANTIGRAVITY_CONFIG_DIR")
        );
        assert_eq!(
            find("opencode").expect("registered").env_override,
            Some("OPENCODE_CONFIG_DIR")
        );
        assert_eq!(
            find("kilo").expect("registered").env_override,
            Some("KILO_CONFIG_DIR")
        );
        assert_eq!(find("copilot").expect("registered").env_override, None);
        assert_eq!(find("hermes").expect("registered").env_override, None);
        assert_eq!(find("grok").expect("registered").env_override, None);
    }
}

/// True when the harness has an MCP adapter, so a scope can be targeted.
pub fn is_mcp_target(id: &str) -> bool {
    HARNESSES
        .iter()
        .any(|harness| harness.id == id && harness.mcp.is_some())
}

/// Every harness the MCP serializer can write, in registry order.
pub fn mcp_targets() -> impl Iterator<Item = &'static HarnessSpec> {
    HARNESSES.iter().filter(|harness| harness.mcp.is_some())
}
