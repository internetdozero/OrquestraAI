//! Blind Credential Vault & MCP blind proxy.
//!
//! Agents invoke HTTP APIs and CLI commands with stored secrets WITHOUT the
//! raw secret ever entering their context: the backend injects the active key
//! in memory (`vault_http_request` / `vault_exec`) and returns scrubbed
//! output. `vault_list_services` exposes masked metadata only.

mod atomic;
mod endpoints;
mod exec;
mod http;
mod presets;
mod proxy;
mod scrub;
mod store;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub use presets::PresetSpec;
pub use store::{mask_key, KeyHealth, VaultServicePublic};

use atomic::update_vault;
pub use proxy::{vault_http_request, vault_test_connection, VaultHttpResult, VaultTestResult};
use store::{
    load_vault, resolve_saved_tag, to_public, validate_id, VaultKey, VaultService,
};

pub use exec::{child_env, scrub_output};
pub use http::{effective_headers, scrub_body, scrub_text, strip_response_headers};

/// Preset catalog for the UI drawer.
#[tauri::command]
pub fn list_vault_presets() -> Vec<PresetSpec> {
    presets::list_presets()
}

/// Masked services: names, active index, counts, health. No secrets.
#[tauri::command]
pub fn vault_list_services() -> Result<Vec<VaultServicePublic>, String> {
    let paths = crate::paths::OrquestraPaths::resolve()?;
    let vault = load_vault(&paths)?;
    Ok(vault.services.values().map(to_public).collect())
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
    validate_id(&id)?;
    let kept: Vec<String> = keys.into_iter().map(|key| key.trim().to_string()).filter(|key| !key.is_empty()).collect();
    if kept.is_empty() {
        return Err("a vault service needs at least one key".to_string());
    }
    let paths = crate::paths::OrquestraPaths::resolve()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    // Snapshot the previous entry for mask/tag preservation WITHOUT holding
    // the vault across the write: the insert below reloads fresh under lock.
    let previous = load_vault(&paths)?.services.remove(&id);
    let mut next_keys: Vec<VaultKey> = Vec::new();
    let wanted_tags: Vec<Option<String>> = key_tags.unwrap_or_default();
    for (index, secret) in kept.iter().enumerate() {
        // Preserve stored secret when the UI sends back its own mask.
        let mut resolved = secret.clone();
        if let Some(prev_key) = previous.as_ref().and_then(|prev| prev.keys.get(index)) {
            if mask_key(&prev_key.secret) == *secret {
                resolved = prev_key.secret.clone();
            }
        }
        let (health, failures, cooldown, prev_tag) = previous
            .as_ref()
            .and_then(|prev| prev.keys.iter().find(|key| key.secret == resolved))
            .map(|key| (key.health, key.failures, key.cooldown_until_ms, key.tag.clone()))
            .unwrap_or((KeyHealth::Healthy, 0, 0, None));
        // Positional fallback for callers that round-trip masks without tags.
        let positional = previous
            .as_ref()
            .and_then(|prev| prev.keys.get(index))
            .and_then(|prev_key| {
                if mask_key(&prev_key.secret) == *secret {
                    prev_key.tag.clone()
                } else {
                    None
                }
            });
        let tag = resolve_saved_tag(wanted_tags.get(index), prev_tag.or(positional));
        next_keys.push(VaultKey {
            id: format!("key-{index}"),
            secret: resolved,
            tag,
            health,
            cooldown_until_ms: cooldown,
            failures,
        });
    }
    let active = previous.as_ref().map(|prev| prev.active.min(next_keys.len().saturating_sub(1))).unwrap_or(0);
    let service = VaultService {
        id: id.clone(),
        label: if label.trim().is_empty() { id.clone() } else { label },
        preset,
        description: description.unwrap_or_default(),
        keys: next_keys,
        active,
        custom_header,
        custom_prefix,
        custom_env,
        meta: meta.unwrap_or_default(),
        updated_at_ms: now,
    };
    let public = to_public(&service);
    update_vault(&paths, |vault| {
        vault.services.insert(id.clone(), service.clone());
        Ok(to_public(&service))
    })?;
    Ok(public)
}

/// Remove one service from the vault.
#[tauri::command]
pub fn vault_delete_service(id: String) -> Result<(), String> {
    let paths = crate::paths::OrquestraPaths::resolve()?;
    update_vault(&paths, |vault| {
        if vault.services.remove(&id).is_none() {
            return Err(format!("unknown vault service: {id}"));
        }
        Ok(())
    })
}

/// Designate the primary key index for a service.
#[tauri::command]
pub fn vault_set_primary(id: String, index: usize) -> Result<VaultServicePublic, String> {
    let paths = crate::paths::OrquestraPaths::resolve()?;
    update_vault(&paths, |vault| {
        let service = vault.services.get_mut(&id).ok_or_else(|| format!("unknown vault service: {id}"))?;
        if index >= service.keys.len() {
            return Err(format!("key index {index} out of range for {id}"));
        }
        service.active = index;
        Ok(to_public(service))
    })
}


/// Blind exec response: exit code + scrubbed output streams.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultExecResult {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub used_key_index: usize,
}

/// Blind `vault_exec`: spawn with env-injected secrets, scrub output.
/// `command` accepts ONLY a string array: `argv[0]` must be a bare name on
/// the exec allowlist (`exec::ALLOWED_BINARIES`) and runs with no shell, so
/// shell strings, pipes and redirections are rejected instead of executed.
#[tauri::command]
pub fn vault_exec(
    service: String,
    command: serde_json::Value,
) -> Result<VaultExecResult, String> {
    let paths = crate::paths::OrquestraPaths::resolve()?;
    let vault = load_vault(&paths)?;
    let current = vault.services.get(&service).ok_or_else(|| format!("unknown vault service: {service}"))?;
    if current.keys.is_empty() {
        return Err(format!("vault service {service} has no keys"));
    }
    let used = current.active.min(current.keys.len().saturating_sub(1));
    let secret = current.keys.get(used).map(|key| key.secret.clone()).unwrap_or_default();
    let (code, stdout, stderr) = exec::run_command(current, &secret, &command)?;
    Ok(VaultExecResult { code, stdout, stderr, used_key_index: used })
}


/// Remove one key from a pool (re-indexes, keeps a valid primary).
#[tauri::command]
pub fn vault_remove_key(id: String, index: usize) -> Result<VaultServicePublic, String> {
    let paths = crate::paths::OrquestraPaths::resolve()?;
    update_vault(&paths, |vault| {
        let service = vault
            .services
            .get_mut(&id)
            .ok_or_else(|| format!("unknown vault service: {id}"))?;
        if service.keys.len() <= 1 {
            return Err(format!("cannot remove the last key of {id}; delete the service instead"));
        }
        if index >= service.keys.len() {
            return Err(format!("key index {index} out of range for {id}"));
        }
        service.keys.remove(index);
        service.active = service.active.min(service.keys.len().saturating_sub(1));
        Ok(to_public(service))
    })
}
