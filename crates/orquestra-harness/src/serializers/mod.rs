//! Adapter serializers: one server definition, one dialect per harness.
//!
//! [`render_mcp_config`] is the single entry point: it picks the dialect from
//! the harness registry's [`McpFormat`] and delegates to [`json`] or [`toml`].
//! Each renderer starts from the config bytes already on disk, so a target
//! that is already in sync round-trips byte for byte.

mod json;
mod toml;

pub use json::{render_jsonc_block, render_mcp_servers};
pub use toml::render_codex_toml;

use std::collections::BTreeSet;

use orquestra_core::harness::McpFormat;

use crate::server::HarnessServer;

/// Render every active server into the envelope `format` describes, removing
/// the ids in `stale` (servers OrquestraAI used to own and no longer wants).
///
/// `entry_type` is the per-entry key some JSON targets require
/// (`type = "stdio"`); dialects that do not take it ignore it.
pub fn render_mcp_config(
    format: McpFormat,
    existing: &str,
    active: &[HarnessServer],
    stale: &BTreeSet<String>,
    entry_type: Option<&str>,
) -> Result<String, String> {
    match format {
        McpFormat::JsonMcpServers => render_mcp_servers(existing, active, stale, entry_type),
        McpFormat::JsoncMcpBlock => render_jsonc_block(existing, active, stale),
        McpFormat::CodexToml => render_codex_toml(existing, active, stale),
    }
}

#[cfg(test)]
mod tests;
