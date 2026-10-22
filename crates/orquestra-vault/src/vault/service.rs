//! Operations behind the desktop shell's IPC commands.
//!
//! Every function takes the resolved storage layout, so the shell keeps only
//! argument marshalling and the `#[tauri::command]` boundary.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use orquestra_core::paths::OrquestraPaths;
use serde::Serialize;

use super::atomic::update_vault;
use super::exec;
use super::store::{
    load_vault, mask_key, purge_migration_backup as purge_store_backup, resolve_saved_tag,
    to_public, validate_id, KeyHealth, VaultKey, VaultService, VaultServicePublic,
};

/// Masked services: names, active index, counts, health. No secrets.
pub fn list_services(paths: &OrquestraPaths) -> Result<Vec<VaultServicePublic>, String> {
    let vault = load_vault(paths)?;
    Ok(vault.services.values().map(to_public).collect())
}

/// Everything needed to register or replace one service.
pub struct SaveServiceInput {
    pub id: String,
    pub label: String,
    pub preset: String,
    pub description: Option<String>,
    /// Raw secrets from the editor; a key equal to the stored mask keeps the
    /// secret already on disk, so the drawer can round-trip safely.
    pub keys: Vec<String>,
    pub custom_header: Option<String>,
    pub custom_prefix: Option<String>,
    pub custom_env: Option<String>,
    pub meta: Option<HashMap<String, String>>,
    pub key_tags: Option<Vec<Option<String>>>,
}

/// Register or replace a service with 1+ keys, preserving stored secrets and
/// key health for anything the editor sent back as a mask.
pub fn save_service(
    paths: &OrquestraPaths,
    input: SaveServiceInput,
) -> Result<VaultServicePublic, String> {
    validate_id(&input.id)?;
    let id = input.id;
    let kept: Vec<String> = input
        .keys
        .into_iter()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
        .collect();
    if kept.is_empty() {
        return Err("a vault service needs at least one key".to_string());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    // Snapshot the previous entry for mask/tag preservation WITHOUT holding
    // the vault across the write: the insert below reloads fresh under lock.
    let previous = load_vault(paths)?.services.remove(&id);
    let mut next_keys: Vec<VaultKey> = Vec::new();
    let wanted_tags: Vec<Option<String>> = input.key_tags.unwrap_or_default();
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
            .map(|key| {
                (
                    key.health,
                    key.failures,
                    key.cooldown_until_ms,
                    key.tag.clone(),
                )
            })
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
    let active = previous
        .as_ref()
        .map(|prev| prev.active.min(next_keys.len().saturating_sub(1)))
        .unwrap_or(0);
    let service = VaultService {
        id: id.clone(),
        label: if input.label.trim().is_empty() {
            id.clone()
        } else {
            input.label
        },
        preset: input.preset,
        description: input.description.unwrap_or_default(),
        keys: next_keys,
        active,
        custom_header: input.custom_header,
        custom_prefix: input.custom_prefix,
        custom_env: input.custom_env,
        meta: input.meta.unwrap_or_default(),
        updated_at_ms: now,
    };
    let public = to_public(&service);
    update_vault(paths, |vault| {
        vault.services.insert(id.clone(), service.clone());
        Ok(to_public(&service))
    })?;
    Ok(public)
}

/// Remove one service from the vault.
pub fn delete_service(paths: &OrquestraPaths, id: &str) -> Result<(), String> {
    update_vault(paths, |vault| {
        if vault.services.remove(id).is_none() {
            return Err(format!("unknown vault service: {id}"));
        }
        Ok(())
    })
}

/// Designate the primary key index for a service.
pub fn set_primary(
    paths: &OrquestraPaths,
    id: &str,
    index: usize,
) -> Result<VaultServicePublic, String> {
    update_vault(paths, |vault| {
        let service = vault
            .services
            .get_mut(id)
            .ok_or_else(|| format!("unknown vault service: {id}"))?;
        if index >= service.keys.len() {
            return Err(format!("key index {index} out of range for {id}"));
        }
        service.active = index;
        Ok(to_public(service))
    })
}

/// Remove one key from a pool (re-indexes, keeps a valid primary).
pub fn remove_key(
    paths: &OrquestraPaths,
    id: &str,
    index: usize,
) -> Result<VaultServicePublic, String> {
    update_vault(paths, |vault| {
        let service = vault
            .services
            .get_mut(id)
            .ok_or_else(|| format!("unknown vault service: {id}"))?;
        if service.keys.len() <= 1 {
            return Err(format!(
                "cannot remove the last key of {id}; delete the service instead"
            ));
        }
        if index >= service.keys.len() {
            return Err(format!("key index {index} out of range for {id}"));
        }
        service.keys.remove(index);
        service.active = service.active.min(service.keys.len().saturating_sub(1));
        Ok(to_public(service))
    })
}

/// Blind exec response: exit code + scrubbed output streams.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultExecResult {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub used_key_index: usize,
}

/// Blind exec: spawn with env-injected secrets, scrub output.
/// `command` accepts ONLY a string array: `argv[0]` must be a bare name on
/// the exec allowlist (`exec::ALLOWED_BINARIES`) and runs with no shell, so
/// shell strings, pipes and redirections are rejected instead of executed.
pub fn execute(
    paths: &OrquestraPaths,
    service: &str,
    command: &serde_json::Value,
) -> Result<VaultExecResult, String> {
    let vault = load_vault(paths)?;
    let current = vault
        .services
        .get(service)
        .ok_or_else(|| format!("unknown vault service: {service}"))?;
    if current.keys.is_empty() {
        return Err(format!("vault service {service} has no keys"));
    }
    let used = current.active.min(current.keys.len().saturating_sub(1));
    let secret = current
        .keys
        .get(used)
        .map(|key| key.secret.clone())
        .unwrap_or_default();
    let (code, stdout, stderr) = exec::run_command(current, &secret, command)?;
    Ok(VaultExecResult {
        code,
        stdout,
        stderr,
        used_key_index: used,
    })
}

/// Schema-level gate: the vault must parse before anything may be purged.
pub fn ensure_readable(paths: &OrquestraPaths) -> Result<(), String> {
    load_vault(paths).map(|_| ())
}

/// Zeroize and unlink the vault's `.migrate-bak` plaintext backup. The
/// sealed-file gate lives in the at-rest layer, so this never destroys the
/// only readable copy.
pub fn purge_migration_backup(paths: &OrquestraPaths) -> Result<bool, String> {
    purge_store_backup(paths)
}
