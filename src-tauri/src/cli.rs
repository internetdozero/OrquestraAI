//! JSON reports for the headless terminal flags in `main.rs`.

use crate::{bridge, mcp, presets};

mod native;
pub use native::{dispatch_native_cli, CliError};

/// JSON snapshot of the bridge for the headless `--bridge-status` flag.
pub fn bridge_status_json() -> Result<String, String> {
    serde_json::to_string_pretty(&bridge::get_bridge_status()?)
        .map_err(|error| format!("failed to serialize bridge status: {error}"))
}

/// Run fusion from a terminal and return its JSON report.
pub fn deploy_bridge_json() -> Result<String, String> {
    serde_json::to_string_pretty(&bridge::deploy_harness_bridge()?)
        .map_err(|error| format!("failed to serialize bridge report: {error}"))
}

/// JSON snapshot of the MCP catalog for the headless `--mcp-status` flag.
pub fn mcp_status_json() -> Result<String, String> {
    serde_json::to_string_pretty(&mcp::list_mcps_masked()?)
        .map_err(|error| format!("failed to serialize MCP catalog: {error}"))
}

/// Toggle one server from a terminal, exactly as the UI switch does
/// (`--mcp-set <id> <global|harness> <on|off>`).
pub fn set_mcp_state_cli(id: &str, scope: &str, enabled: bool) -> Result<String, String> {
    let harness = if scope == "global" { None } else { Some(scope) };
    mcp::set_mcp_state(id, harness, enabled)?;

    serde_json::to_string_pretty(&serde_json::json!({
        "id": id,
        "scope": scope,
        "enabled": enabled,
    }))
    .map_err(|error| format!("failed to serialize MCP state report: {error}"))
}

/// Apply a preset from a terminal (`--preset <id>`) and return its report.
pub fn apply_preset_json(id: &str) -> Result<String, String> {
    serde_json::to_string_pretty(&presets::apply_preset(id.to_string())?)
        .map_err(|error| format!("failed to serialize the preset report: {error}"))
}

/// Run the MCP serializer from a terminal and report which files changed.
pub fn deploy_mcp_json() -> Result<String, String> {
    let changed = mcp::deploy_mcps_reporting()?;
    let targets = mcp::list_targets()?;

    serde_json::to_string_pretty(&serde_json::json!({
        "changed": changed,
        "targets": targets.len(),
        "servers": mcp::list_mcps()?.len(),
    }))
    .map_err(|error| format!("failed to serialize MCP deploy report: {error}"))
}
