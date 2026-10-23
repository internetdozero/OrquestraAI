#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

mod at_rest;
pub mod bridge;
mod cli;
mod community;
mod desktop;
mod desktop_backup;
mod desktop_icons;
// `paths` and `harness` are re-exports of `orquestra-core` (task #326); the
// items moved there, the import paths did not.
pub mod harness;
pub mod ideas;
#[cfg(test)]
mod ipc_acl;
#[cfg(test)]
mod ipc_contract;
pub mod janitor;
pub mod mcp;
mod official;
mod official_mcps;
pub mod paths;
pub mod presets;
pub mod sessions;
pub mod single_instance;
mod skills;
mod tray;
pub mod update_sig;
mod updater;
pub mod vault;

// Logging lives in `orquestra-core` so every crate emits the same line format;
// re-exported here so each `crate::log_warn!` / `log_error!` / `log_info!`
// call site keeps working unchanged.
pub use orquestra_core::{log_error, log_info, log_warn};

use crate::paths::OrquestraPaths;
use bridge::{BridgeReport, HarnessStatus};
use desktop::{create_orquestra_backup, open_in_file_manager, open_url};
use ideas::{
    create_idea, delete_idea, get_productivity_metrics, list_blackboard_notes, list_idea_projects,
    list_ideas, mark_note_read, post_blackboard_note, update_idea, update_idea_status,
};
use janitor::{get_system_health, sweep_system_caches};
use mcp::{McpServer, McpTargetStatus};
use official::OfficialStore;
use serde::Serialize;
use sessions::{
    estimate_purge, get_retention_policy, get_session_transcript, get_unified_sessions,
    purge_sessions, save_retention_policy, toggle_pin_session,
};
use skills::{HarnessSummary, SkillItem};
use tauri::Manager;
use tray::set_minimize_to_tray;
use updater::{restart_app, sync_official_extensions};
// Re-exported (not just a Tauri command) so integration tests can prove the
// updater fails closed on hostile versions without touching the network.
pub use updater::apply_app_update;
use vault::{
    list_vault_presets, vault_delete_service, vault_exec, vault_http_request, vault_list_services,
    vault_purge_migration_backups, vault_remove_key, vault_save_service, vault_set_primary,
    vault_test_connection,
};

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

/// Crash reports from the React error boundaries land on stderr next to the
/// backend logs, so a white-screen report carries its area and message.
#[tauri::command]
fn log_ui_error(area: String, message: String, stack: Option<String>) -> Result<(), String> {
    crate::log_error!("ui error in {area}: {message}");
    if let Some(stack) = stack.filter(|stack| !stack.is_empty()) {
        crate::log_error!("ui stack: {stack}");
    }
    Ok(())
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

/// Read the raw `SKILL.md` of one vault skill, for the in-app editor.
#[tauri::command]
fn read_skill_content(id: String) -> Result<String, String> {
    skills::read_skill_content(id)
}

/// Atomically replace a skill's `SKILL.md`.
#[tauri::command]
fn save_skill_content(id: String, content: String) -> Result<(), String> {
    skills::save_skill_content(id, content)
}

/// Unlink a skill from every scope and remove its vault folder.
#[tauri::command]
fn delete_skill(id: String) -> Result<(), String> {
    skills::delete_skill(id)
}

/// Copy an external skill folder into the vault and return its item.
#[tauri::command]
fn import_skill(source_path: String) -> Result<SkillItem, String> {
    skills::import_skill(source_path)
}

/// Shallow-clone a git URL and import the skill it holds.
#[tauri::command]
fn import_skill_from_git(url: String) -> Result<SkillItem, String> {
    skills::import_skill_from_git(url)
}

/// Open a native OS folder picker; `None` means the user cancelled.
#[tauri::command]
async fn pick_folder() -> Result<Option<String>, String> {
    skills::pick_folder().await
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
    mcp::list_mcps_masked()
}

/// Curated Official Store: designated skills and MCP servers with install
/// sources. Static data, so infallible by construction.
#[tauri::command]
fn list_official_store() -> OfficialStore {
    official::official_store()
}

/// Reveal one stored MCP environment/header value after an explicit UI action.
#[tauri::command]
fn reveal_mcp_secret(id: String, key: String) -> Result<String, String> {
    mcp::reveal_mcp_secret(&id, &key)
}

/// Harness configs the MCP serializer can write.
#[tauri::command]
fn mcp_targets() -> Result<Vec<McpTargetStatus>, String> {
    mcp::list_targets()
}

/// Project or un-project one MCP server globally or for a single harness.
#[tauri::command]
fn set_mcp_state(id: String, harness_id: Option<String>, enabled: bool) -> Result<(), String> {
    mcp::set_mcp_state(&id, harness_id.as_deref(), enabled)
}

/// Enable or disable every MCP server for one scope in a single atomic write.
#[tauri::command]
fn batch_set_mcp_all(harness_id: Option<String>, enabled: bool) -> Result<(), String> {
    mcp::batch_set_mcp_all(harness_id.as_deref(), enabled)
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
fn save_preset(name: String, description: Option<String>) -> Result<presets::Preset, String> {
    presets::save_preset(name, description)
}

/// Remove one custom preset. Built-ins cannot be deleted.
#[tauri::command]
fn delete_preset(id: String) -> Result<(), String> {
    presets::delete_preset(&id)
}

/// Update an existing preset (custom or built-in override).
#[tauri::command]
fn update_preset(preset: presets::Preset) -> Result<presets::Preset, String> {
    presets::update_preset(preset)
}

/// Reset a built-in preset to original factory defaults.
#[tauri::command]
fn reset_preset(id: String) -> Result<presets::Preset, String> {
    presets::reset_preset(&id)
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

pub use cli::{
    apply_preset_json, bridge_status_json, deploy_bridge_json, deploy_mcp_json,
    dispatch_native_cli, mcp_status_json, set_mcp_state_cli, CliError,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "linux")]
            if let Some(window) = app.get_webview_window("main") {
                window.set_background_color(Some(tauri::window::Color(9, 9, 11, 255)))?;
            }
            match OrquestraPaths::resolve().and_then(|paths| paths.bootstrap()) {
                Ok(()) => {}
                Err(error) => crate::log_error!("storage bootstrap failed: {error}"),
            }

            // Desktop entry and icon integration for Linux application launchers and docks.
            desktop::ensure_desktop_integration();

            // Panel presence. A host without a tray must still get a window,
            // so a failure here is reported and the app carries on.
            if let Err(error) = tray::setup_tray(app.handle()) {
                crate::log_warn!("system tray unavailable: {error}");
            }

            // Single instance listener: ensures subsequent launches focus this window
            // rather than spawning duplicate processes and duplicate tray icons.
            if let Err(error) = single_instance::start_listener(app.handle().clone()) {
                crate::log_warn!("single-instance listener failed: {error}");
            }

            // Retention daemon. It enforces nothing unless the operator enabled
            // auto-purge, and it never touches a pinned or open session.
            sessions::start_scheduler();

            // Close behavior the settings UI keeps in sync (`minimizeToTray`).
            app.manage(tray::AppConfigState::default());

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                tray::on_close_requested(window, api);
            }
        })
        .invoke_handler(tauri::generate_handler![
            core_status,
            log_ui_error,
            get_system_health,
            sweep_system_caches,
            list_skills,
            set_skill_state,
            batch_set_all,
            read_skill_content,
            save_skill_content,
            delete_skill,
            import_skill,
            import_skill_from_git,
            pick_folder,
            open_url,
            open_in_file_manager,
            create_orquestra_backup,
            set_minimize_to_tray,
            harness_summary,
            read_master_agents_md,
            write_master_agents_md,
            get_bridge_status,
            deploy_harness_bridge,
            list_mcps,
            list_official_store,
            reveal_mcp_secret,
            mcp_targets,
            set_mcp_state,
            batch_set_mcp_all,
            save_mcp,
            delete_mcp,
            deploy_mcps,
            list_presets,
            apply_preset,
            save_preset,
            delete_preset,
            update_preset,
            reset_preset,
            list_ideas,
            list_idea_projects,
            get_productivity_metrics,
            create_idea,
            update_idea,
            update_idea_status,
            delete_idea,
            list_blackboard_notes,
            post_blackboard_note,
            mark_note_read,
            get_unified_sessions,
            get_session_transcript,
            toggle_pin_session,
            get_retention_policy,
            save_retention_policy,
            estimate_purge,
            purge_sessions,
            apply_app_update,
            sync_official_extensions,
            restart_app,
            list_vault_presets,
            vault_list_services,
            vault_save_service,
            vault_delete_service,
            vault_set_primary,
            vault_http_request,
            vault_exec,
            vault_test_connection,
            vault_remove_key,
            vault_purge_migration_backups
        ])
        .run(tauri::generate_context!())
    {
        crate::log_error!("error while running OrquestraAI: {error}");
    }
}
