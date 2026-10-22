//! Blind Credential Vault & MCP blind proxy — the IPC layer.
//!
//! The store, the blind HTTP/exec proxies and the at-rest encryption live in
//! `orquestra-vault` (task #328). What stays here are the `#[tauri::command]`
//! wrappers and the re-exports that keep `crate::vault::…` unchanged for the
//! rest of the shell.
//!
//! Agents invoke HTTP APIs and CLI commands with stored secrets WITHOUT the
//! raw secret ever entering their context: the backend injects the active key
//! in memory (`vault_http_request` / `vault_exec`) and returns scrubbed
//! output. `vault_list_services` exposes masked metadata only.

use std::collections::HashMap;

use orquestra_vault::vault::{presets, proxy, service};

pub use orquestra_vault::vault::exec::{child_env, scrub_output};
pub use orquestra_vault::vault::http::{
    effective_headers, scrub_body, scrub_text, strip_response_headers,
};
pub use orquestra_vault::vault::presets::PresetSpec;
pub use orquestra_vault::vault::proxy::{VaultHttpResult, VaultTestResult};
pub use orquestra_vault::vault::service::{SaveServiceInput, VaultExecResult};
pub use orquestra_vault::vault::{mask_key, KeyHealth, VaultServicePublic};

use crate::paths::OrquestraPaths;

/// Preset catalog for the UI drawer.
#[tauri::command]
pub fn list_vault_presets() -> Vec<PresetSpec> {
    presets::list_presets()
}

/// Masked services: names, active index, counts, health. No secrets.
#[tauri::command]
pub fn vault_list_services() -> Result<Vec<VaultServicePublic>, String> {
    let paths = OrquestraPaths::resolve()?;
    service::list_services(&paths)
}

/// Register or replace a service with 1+ keys. Masked placeholders for
/// existing keys are preserved so the drawer can round-trip safely.
#[tauri::command]
pub fn vault_save_service(
    id: String,
    label: String,
    preset: String,
    description: Option<String>,
    keys: Vec<String>,
    custom_header: Option<String>,
    custom_prefix: Option<String>,
    custom_env: Option<String>,
    meta: Option<HashMap<String, String>>,
    key_tags: Option<Vec<Option<String>>>,
) -> Result<VaultServicePublic, String> {
    let paths = OrquestraPaths::resolve()?;
    service::save_service(
        &paths,
        SaveServiceInput {
            id,
            label,
            preset,
            description,
            keys,
            custom_header,
            custom_prefix,
            custom_env,
            meta,
            key_tags,
        },
    )
}

/// Remove one service from the vault.
#[tauri::command]
pub fn vault_delete_service(id: String) -> Result<(), String> {
    let paths = OrquestraPaths::resolve()?;
    service::delete_service(&paths, &id)
}

/// Designate the primary key index for a service.
#[tauri::command]
pub fn vault_set_primary(id: String, index: usize) -> Result<VaultServicePublic, String> {
    let paths = OrquestraPaths::resolve()?;
    service::set_primary(&paths, &id, index)
}

/// Blind `vault_http_request`: inject active key, fail over on 401/402/403/
/// 429, strip auth response headers, scrub secrets from the body.
#[tauri::command]
pub async fn vault_http_request(
    service: String,
    endpoint: String,
    method: Option<String>,
    headers: Option<HashMap<String, String>>,
    body: Option<String>,
    key_tag: Option<String>,
) -> Result<VaultHttpResult, String> {
    proxy::request(service, endpoint, method, headers, body, key_tag).await
}

/// Blind `vault_test_connection`: ping the preset's verify endpoint with one
/// pooled key, persist the observed health, return latency + outcome. The
/// secret never appears in the result.
#[tauri::command]
pub async fn vault_test_connection(
    service: String,
    key_index: Option<usize>,
) -> Result<VaultTestResult, String> {
    proxy::test_connection(service, key_index).await
}

/// Blind `vault_exec`: spawn with env-injected secrets, scrub output.
/// `command` accepts ONLY a string array: `argv[0]` must be a bare name on
/// the exec allowlist (`exec::ALLOWED_BINARIES`) and runs with no shell, so
/// shell strings, pipes and redirections are rejected instead of executed.
#[tauri::command]
pub fn vault_exec(service: String, command: serde_json::Value) -> Result<VaultExecResult, String> {
    let paths = OrquestraPaths::resolve()?;
    orquestra_vault::vault::service::execute(&paths, &service, &command)
}

/// Per-file outcome of a migration-backup purge: both secret-bearing files
/// (`vault.json`, `mcps.json`) may keep a `.migrate-bak` plaintext backup.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeBackupsReport {
    pub vault_purged: bool,
    pub catalog_purged: bool,
}

/// Zeroize and unlink the `.migrate-bak` plaintext backups left by the
/// at-rest encryption migration. Runs only after both sealed files prove
/// they still open (schema-level reload here, unseal gate inside the purge
/// itself), so this never destroys the only readable copy. The backups are
/// kept until the operator explicitly purges them — auto-deleting on first
/// write would strand a user whose new key cannot open the sealed files.
#[tauri::command]
pub fn vault_purge_migration_backups() -> Result<PurgeBackupsReport, String> {
    let paths = OrquestraPaths::resolve()?;
    service::ensure_readable(&paths)?;
    crate::mcp::list_mcps()?;
    let vault_purged = service::purge_migration_backup(&paths)?;
    let catalog_purged = crate::mcp::purge_catalog_migration_backup()?;
    Ok(PurgeBackupsReport {
        vault_purged,
        catalog_purged,
    })
}

/// Remove one key from a pool (re-indexes, keeps a valid primary).
#[tauri::command]
pub fn vault_remove_key(id: String, index: usize) -> Result<VaultServicePublic, String> {
    let paths = OrquestraPaths::resolve()?;
    service::remove_key(&paths, &id, index)
}
