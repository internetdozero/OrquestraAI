//! The harness table itself: one entry per administered harness.

use super::{HarnessSpec, McpFormat, McpSpec, SecondaryAction, SecondarySpec};

/// The `skills` CLI installs into a shared user root that Codex and Grok both
/// inject into their session context. Freebuff owns that root as its canonical
/// projection, so the secondary entries below only apply while Freebuff is
/// absent; fusion defers to the owner instead of emptying the link.
const SHARED_AGENT_SKILLS: SecondarySpec = SecondarySpec {
    rel: ".agents/skills",
    action: SecondaryAction::Quarantine,
    rationale: "Shared root Codex and Grok also read; Freebuff links it to its projection",
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
        // `~/.agents/skills` is the Codebuff SDK's canonical global root, so
        // Freebuff reads operator-enabled skills there before its own folders.
        skills_links: &[
            ".agents/skills",
            ".freebuff/skills",
            ".config/manicode/skills",
        ],
        instructions_rel: Some(".config/manicode/AGENTS.md"),
        config_rel: None,
        mcp: None,
        secondary: &[],
        note: Some("Manicode and ~/.agents/skills share the Freebuff projection"),
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
