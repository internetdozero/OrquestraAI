use std::path::Path;
use std::process::Command;

fn main() {
    let profile = std::env::var("PROFILE").unwrap_or_default();

    // Check whether this build was invoked via Tauri CLI or with custom-protocol
    let has_custom_protocol = std::env::var("CARGO_FEATURE_CUSTOM_PROTOCOL").is_ok();
    let is_tauri_cli = std::env::vars().any(|(k, _)| k.starts_with("TAURI_"));

    if profile == "release" {
        // Block raw cargo builds that lack both custom-protocol and Tauri CLI context
        if !has_custom_protocol && !is_tauri_cli {
            panic!(
                "\n\n\
                ========================================================================\n\
                ❌ [OrquestraAI Guardrail] Direct 'cargo build --release' is forbidden!\n\
                ========================================================================\n\
                Direct cargo build does not embed the frontend assets properly and\n\
                causes the release binary to fail at runtime with:\n\
                'Could not connect to 127.0.0.1: Connection refused'.\n\n\
                👉 To build the release executable, run:\n\
                   npm run build:release\n\
                ========================================================================\n\n"
            );
        }

        // Guardrail 2: Ensure frontend assets exist in ../dist/
        let dist_index = Path::new("../dist/index.html");
        if !dist_index.exists() {
            println!("cargo:warning=dist/index.html is missing! Triggering 'npm run build' automatically...");
            let npm_bin = resolve_npm_command().unwrap_or_else(|| {
                panic!(
                    "\n\n❌ [OrquestraAI Guardrail]\n\
                    Frontend assets are required for release builds, but 'npm' was not found in PATH!\n\
                    Please install Node.js/npm or build the frontend manually ('npm run build').\n"
                );
            });

            let status = Command::new(&npm_bin)
                .args(["run", "build"])
                .current_dir("..")
                .status();

            match status {
                Ok(s) if s.success() => {
                    println!("cargo:warning=Frontend built successfully via automatic guardrail.");
                }
                _ => {
                    panic!(
                        "\n\n❌ [OrquestraAI Guardrail]\n\
                        Frontend assets are required for release builds, but 'npm run build' failed!\n\
                        Build aborted to prevent generating a broken binary.\n\
                        Please fix frontend build errors or run 'npm run build' manually.\n"
                    );
                }
            }
        }
    }

    // Every `#[tauri::command]` in the invoke handler is declared here so the
    // ACL denies it by default. Each command becomes an `allow-<command>` /
    // `deny-<command>` permission; the `permissions/*.toml` scope sets grant
    // them back per capability file. Removing a scope file (e.g. vault-ops)
    // makes those invokes fail closed with a permission error instead of
    // silently staying callable. Keep this list in sync with the
    // `generate_handler!` list in `src/lib.rs` — the `capability_scopes_*`
    // tests enforce the match.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "core_status",
            "log_ui_error",
            "get_system_health",
            "sweep_system_caches",
            "list_skills",
            "set_skill_state",
            "batch_set_all",
            "read_skill_content",
            "save_skill_content",
            "delete_skill",
            "import_skill",
            "import_skill_from_git",
            "pick_folder",
            "open_url",
            "open_in_file_manager",
            "create_orquestra_backup",
            "set_minimize_to_tray",
            "harness_summary",
            "read_master_agents_md",
            "write_master_agents_md",
            "get_bridge_status",
            "deploy_harness_bridge",
            "list_mcps",
            "list_official_store",
            "install_rtk",
            "rtk_status",
            "reveal_mcp_secret",
            "mcp_targets",
            "set_mcp_state",
            "batch_set_mcp_all",
            "save_mcp",
            "delete_mcp",
            "deploy_mcps",
            "list_presets",
            "apply_preset",
            "save_preset",
            "delete_preset",
            "update_preset",
            "reset_preset",
            "list_ideas",
            "list_idea_projects",
            "get_productivity_metrics",
            "create_idea",
            "update_idea",
            "update_idea_status",
            "delete_idea",
            "list_blackboard_notes",
            "post_blackboard_note",
            "mark_note_read",
            "get_unified_sessions",
            "get_session_transcript",
            "toggle_pin_session",
            "get_retention_policy",
            "save_retention_policy",
            "estimate_purge",
            "purge_sessions",
            "apply_app_update",
            "sync_official_extensions",
            "restart_app",
            "list_vault_presets",
            "vault_list_services",
            "vault_import_services",
            "vault_reveal_key",
            "vault_save_service",
            "vault_delete_service",
            "vault_set_service_enabled",
            "vault_set_primary",
            "vault_http_request",
            "vault_exec",
            "vault_test_connection",
            "vault_remove_key",
            "vault_purge_migration_backups",
        ]),
    ))
    .expect("tauri build failed");
}

fn resolve_npm_command() -> Option<std::path::PathBuf> {
    let path = Path::new("npm");
    if is_executable(path) {
        return Some(path.to_path_buf());
    }
    let search = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&search) {
        let candidate = directory.join("npm");
        if is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        for suffix in [".cmd", ".bat", ".exe"] {
            let candidate = directory.join(format!("npm{suffix}"));
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file())
        .unwrap_or(false)
}
