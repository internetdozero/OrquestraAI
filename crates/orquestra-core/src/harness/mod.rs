//! Canonical registry of every AI harness OrquestraAI administers.
//!
//! One entry per harness: its id and label, the directory that proves it
//! is installed, the skill directories and instruction file fusion links,
//! the JSONC config that receives `skills.paths`, the MCP envelope it
//! expects, and the secondary discovery locations that bypass its
//! projection. `skills`, `mcp` and `bridge` all read this table, so a
//! harness is described in exactly one place.
//!
//! The table itself lives in [`registry`]; the queries over it live in
//! [`lookup`]. Both are re-exported here, so callers keep using
//! `orquestra_core::harness::find(..)` and friends.

mod lookup;
mod registry;

pub use lookup::{
    find, harness_ids, is_installed, is_installed_with, is_mcp_target, mcp_targets, primary_owner,
    provisions_parent, resolve_install_dir, resolve_install_dir_with,
};
pub use registry::HARNESSES;

use serde::Serialize;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    /// `CODEX_HOME` set to the mock path, everything else absent.
    fn codex_home_override(key: &str) -> Option<String> {
        (key == "CODEX_HOME").then(|| "/tmp/orquestra-codex-mock".to_string())
    }

    /// An empty environment: every override falls back to the home default.
    fn no_overrides(_key: &str) -> Option<String> {
        None
    }

    /// `CODEX_HOME` wins over `~/.codex` while set; an absent variable
    /// restores the home-relative default, and `is_installed` follows the
    /// resolved path. The environment is injected, so the test never touches
    /// the process-global variable that other threads read.
    #[test]
    fn env_override_wins_and_falls_back() {
        let spec = find("codex").expect("codex is registered");
        assert_eq!(spec.env_override, Some("CODEX_HOME"));

        let home = std::env::temp_dir().join(format!("orquestra-harness-{}", std::process::id()));

        assert_eq!(
            resolve_install_dir_with(spec, &home, &codex_home_override),
            PathBuf::from("/tmp/orquestra-codex-mock")
        );

        assert_eq!(
            resolve_install_dir_with(spec, &home, &no_overrides),
            home.join(".codex")
        );
        assert!(!is_installed_with(spec, &home, &no_overrides));

        std::fs::create_dir_all(home.join(".codex")).unwrap();
        assert!(is_installed_with(spec, &home, &no_overrides));
        std::fs::remove_dir_all(&home).unwrap();
        assert!(!is_installed_with(spec, &home, &no_overrides));
    }

    #[test]
    fn empty_override_falls_back_to_home() {
        let spec = find("claude").expect("claude is registered");
        assert_eq!(spec.env_override, Some("CLAUDE_CONFIG_DIR"));
        let home = Path::new("/tmp/orquestra-home-that-cannot-exist");

        // Whitespace-only is the "set but empty" case: it must not win.
        let whitespace = |key: &str| (key == "CLAUDE_CONFIG_DIR").then(|| "   ".to_string());
        assert_eq!(
            resolve_install_dir_with(spec, home, &whitespace),
            home.join(".claude")
        );
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
