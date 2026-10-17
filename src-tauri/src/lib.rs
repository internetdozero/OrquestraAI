pub mod bridge;
pub mod harness;
#[cfg(test)]
mod ipc_contract;
pub mod mcp;
pub mod paths;
pub mod presets;
pub mod single_instance;
mod skills;
mod tray;

use bridge::{BridgeReport, HarnessStatus};
use mcp::{McpServer, McpTargetStatus};
use crate::paths::OrquestraPaths;
use serde::Serialize;
use skills::{HarnessSummary, SkillItem};
use tauri::Manager;

/// Runtime identity of the native core, surfaced in the UI footer so the
/// frontend can prove the IPC bridge is alive.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreStatus {
    name: String,
    version: String,
    platform: String,
    bridge: String,
}

#[tauri::command]
fn core_status(app: tauri::AppHandle) -> CoreStatus {
    let package = app.package_info();

    CoreStatus {
        name: package.name.clone(),
        version: package.version.to_string(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        bridge: "ready".to_string(),
    }
}

/// Scan the master vault and evaluate each skill for the selected harness.
/// `None` evaluates the universal Global baseline.
#[tauri::command]
fn list_skills(harness_id: Option<String>) -> Result<Vec<SkillItem>, String> {
    skills::list_skills(harness_id)
}

/// Project or un-project one skill. A `global` scope propagates to every
/// harness projection folder; harness scopes stay isolated.
#[tauri::command]
fn set_skill_state(id: String, harness_id: String, enabled: bool) -> Result<(), String> {
    skills::set_skill_state(&id, &harness_id, enabled)
}

/// Project or un-project every skill for one scope.
#[tauri::command]
fn batch_set_all(harness_id: String, enabled: bool) -> Result<(), String> {
    skills::batch_set_all(harness_id, enabled)
}

/// Active-skill counters for Global and every harness tab.
#[tauri::command]
fn harness_summary() -> Result<Vec<HarnessSummary>, String> {
    skills::harness_summary()
}

/// Read the master `~/.orquestra/AGENTS.md`, creating a template on first use.
#[tauri::command]
fn read_master_agents_md() -> Result<String, String> {
    skills::read_master_agents_md()
}

/// Atomically replace the master `~/.orquestra/AGENTS.md`.
#[tauri::command]
fn write_master_agents_md(content: String) -> Result<(), String> {
    skills::write_master_agents_md(&content)
}

/// Every MCP server in the master catalog, seeded on first launch.
#[tauri::command]
fn list_mcps() -> Result<Vec<McpServer>, String> {
    mcp::list_mcps()
}

/// Harness configs the MCP serializer can write.
#[tauri::command]
fn mcp_targets() -> Result<Vec<McpTargetStatus>, String> {
    mcp::list_targets()
}

/// Project or un-project one MCP server globally or for a single harness.
#[tauri::command]
fn set_mcp_state(
    id: String,
    harness_id: Option<String>,
    enabled: bool,
) -> Result<(), String> {
    mcp::set_mcp_state(&id, harness_id.as_deref(), enabled)
}

/// Create or replace one MCP server, then re-compile every harness config.
#[tauri::command]
fn save_mcp(mcp_server: McpServer) -> Result<(), String> {
    mcp::save_mcp(mcp_server)
}

/// Remove one MCP server from the catalog and from every harness config.
#[tauri::command]
fn delete_mcp(id: String) -> Result<(), String> {
    mcp::delete_mcp(&id)
}

/// Re-compile the catalog into every installed harness config.
#[tauri::command]
fn deploy_mcps() -> Result<Vec<String>, String> {
    mcp::deploy_mcps_reporting()
}

/// Every built-in and custom mission preset, with the applied one flagged.
#[tauri::command]
fn list_presets() -> Result<Vec<presets::Preset>, String> {
    presets::list_presets()
}

/// Apply one preset to the skill projections and the MCP catalog.
#[tauri::command]
fn apply_preset(id: String) -> Result<presets::PresetReport, String> {
    presets::apply_preset(id)
}

/// Save the current live skill and MCP combination as a named custom preset.
#[tauri::command]
fn save_preset(
    name: String,
    description: Option<String>,
) -> Result<presets::Preset, String> {
    presets::save_preset(name, description)
}

/// Remove one custom preset. Built-ins cannot be deleted.
#[tauri::command]
fn delete_preset(id: String) -> Result<(), String> {
    presets::delete_preset(&id)
}

/// Live link state for every known harness, used by the footer indicator.
#[tauri::command]
fn get_bridge_status() -> Result<Vec<HarnessStatus>, String> {
    bridge::get_bridge_status()
}

/// Back up each harness, then symlink it into its OrquestraAI projection.
#[tauri::command]
fn deploy_harness_bridge() -> Result<BridgeReport, String> {
    bridge::deploy_harness_bridge()
}

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
    serde_json::to_string_pretty(&mcp::list_mcps()?)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            match OrquestraPaths::resolve().and_then(|paths| paths.bootstrap()) {
                Ok(()) => {}
                Err(error) => eprintln!("[orquestra] storage bootstrap failed: {error}"),
            }

            // Panel presence. A host without a tray must still get a window,
            // so a failure here is reported and the app carries on.
            if let Err(error) = tray::setup_tray(app.handle()) {
                eprintln!("[orquestra] system tray unavailable: {error}");
            }

            // Single instance listener: ensures subsequent launches focus this window
            // rather than spawning duplicate processes and duplicate tray icons.
            if let Err(error) = single_instance::start_listener(app.handle().clone()) {
                eprintln!("[orquestra] single-instance listener failed: {error}");
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                single_instance::cleanup();
                window.app_handle().exit(0);
            }
        })
        .invoke_handler(tauri::generate_handler![
            core_status,
            list_skills,
            set_skill_state,
            batch_set_all,
            harness_summary,
            read_master_agents_md,
            write_master_agents_md,
            get_bridge_status,
            deploy_harness_bridge,
            list_mcps,
            mcp_targets,
            set_mcp_state,
            save_mcp,
            delete_mcp,
            deploy_mcps,
            list_presets,
            apply_preset,
            save_preset,
            delete_preset
        ])
        .run(tauri::generate_context!())
        .expect("error while running OrquestraAI");
}
