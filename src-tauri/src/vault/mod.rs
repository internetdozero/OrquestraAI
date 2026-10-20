//! Blind Credential Vault & MCP blind proxy.
//!
//! Agents invoke HTTP APIs and CLI commands with stored secrets WITHOUT the
//! raw secret ever entering their context: the backend injects the active key
//! in memory (`vault_http_request` / `vault_exec`) and returns scrubbed
//! output. `vault_list_services` exposes masked metadata only.

mod exec;
mod http;
mod presets;
mod store;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub use presets::PresetSpec;
pub use store::{mask_key, KeyHealth, VaultServicePublic};

use store::{
    first_key_index_in, load_vault, next_key_index_in, normalize_tag, record_result,
    record_result_in, resolve_saved_tag, save_vault, service_health, to_public, validate_id,
    VaultKey, VaultService,
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
    let mut vault = load_vault(&paths)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let previous = vault.services.get(&id);
    let mut next_keys: Vec<VaultKey> = Vec::new();
    let wanted_tags: Vec<Option<String>> = key_tags.unwrap_or_default();
    for (index, secret) in kept.iter().enumerate() {
        // Preserve stored secret when the UI sends back its own mask.
        let mut resolved = secret.clone();
        if let Some(prev) = previous {
            if let Some(prev_key) = prev.keys.get(index) {
                if mask_key(&prev_key.secret) == *secret {
                    resolved = prev_key.secret.clone();
                }
            }
        }
        let (health, failures, cooldown, prev_tag) = previous
            .and_then(|prev| prev.keys.iter().find(|key| key.secret == resolved))
            .map(|key| (key.health, key.failures, key.cooldown_until_ms, key.tag.clone()))
            .unwrap_or((KeyHealth::Healthy, 0, 0, None));
        // Positional fallback for callers that round-trip masks without tags.
        let positional = previous
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
    let active = previous.map(|prev| prev.active.min(next_keys.len().saturating_sub(1))).unwrap_or(0);
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
    vault.services.insert(id, service);
    save_vault(&paths, &vault)?;
    Ok(public)
}

/// Remove one service from the vault.
#[tauri::command]
pub fn vault_delete_service(id: String) -> Result<(), String> {
    let paths = crate::paths::OrquestraPaths::resolve()?;
    let mut vault = load_vault(&paths)?;
    if vault.services.remove(&id).is_none() {
        return Err(format!("unknown vault service: {id}"));
    }
    save_vault(&paths, &vault)
}

/// Designate the primary key index for a service.
#[tauri::command]
pub fn vault_set_primary(id: String, index: usize) -> Result<VaultServicePublic, String> {
    let paths = crate::paths::OrquestraPaths::resolve()?;
    let mut vault = load_vault(&paths)?;
    let service = vault.services.get_mut(&id).ok_or_else(|| format!("unknown vault service: {id}"))?;
    if index >= service.keys.len() {
        return Err(format!("key index {index} out of range for {id}"));
    }
    service.active = index;
    let public = to_public(service);
    save_vault(&paths, &vault)?;
    Ok(public)
}

/// Blind HTTP response: status, filtered headers, scrubbed body. The secret
/// never appears in the payload.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultHttpResult {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub used_key_index: usize,
    pub rotated: bool,
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
    if endpoint.trim().is_empty() {
        return Err("vault http needs an endpoint URL".to_string());
    }
    if !(endpoint.starts_with("https://") || endpoint.starts_with("http://")) {
        return Err("vault endpoint must start with http(s)://".to_string());
    }
    let verb = method.unwrap_or_else(|| "GET".to_string());
    let extra = headers.unwrap_or_default();
    let tag = key_tag.as_deref().and_then(normalize_tag);
    let paths = crate::paths::OrquestraPaths::resolve()?;
    let mut vault = store::load_vault(&paths)?;
    let snapshot = vault.services.get(&service).cloned().ok_or_else(|| format!("unknown vault service: {service}"))?;
    if snapshot.keys.is_empty() {
        return Err(format!("vault service {service} has no keys"));
    }
    if let Some(wanted) = tag.as_deref() {
        let matches = snapshot.keys.iter().filter(|key| {
            key.tag.as_deref().map(|raw| raw.trim().to_lowercase()).as_deref() == Some(wanted)
        }).count();
        if matches == 0 {
            return Err(format!("vault service {service} has no key tagged \"{wanted}\""));
        }
    }
    let scoped_len = match tag.as_deref() {
        None => snapshot.keys.len(),
        Some(wanted) => snapshot.keys.iter().filter(|key| {
            key.tag.as_deref().map(|raw| raw.trim().to_lowercase()).as_deref() == Some(wanted)
        }).count().max(1),
    };
    let attempts = scoped_len.max(1);
    let now_ms = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let start = snapshot.active.min(snapshot.keys.len().saturating_sub(1));
    let mut used = first_key_index_in(&snapshot.keys, start, now_ms(), tag.as_deref())
        .unwrap_or(start);
    let mut first_used = used;
    let mut last_error = String::new();
    for _ in 0..attempts {
        let current = vault.services.get(&service).ok_or_else(|| format!("unknown vault service: {service}"))?.clone();
        let secret = current.keys.get(used).map(|key| key.secret.clone()).ok_or_else(|| format!("vault service {service} has no key at {used}"))?;
        let outgoing = http::effective_headers(&current, &secret, &extra);
        match http::execute_once(&verb, &endpoint, &outgoing, body.as_deref()).await {
            Ok((status, headers, text)) => {
                let secrets: Vec<String> = current.keys.iter().map(|key| key.secret.clone()).collect();
                let scrubbed = http::scrub_text(&text, &secrets);
                let rotated = used != first_used;
                {
                    let live = vault.services.get_mut(&service).ok_or_else(|| format!("unknown vault service: {service}"))?;
                    record_result_in(live, used, status, tag.as_deref());
                    let _ = save_vault(&paths, &vault);
                }
                // Fail over transparently: retry once with the rotated key.
                if store::health_for_status(status).is_some() {
                    if let Some(live) = vault.services.get(&service) {
                        if live.active != used {
                            used = live.active;
                            first_used = first_used.min(used);
                            continue;
                        }
                    }
                }
                return Ok(VaultHttpResult { status, headers, body: scrubbed, used_key_index: used, rotated });
            }
            Err(error) => {
                last_error = error;
                if let Some(live) = vault.services.get(&service) {
                    if let Some(next) = next_key_index_in(&live.keys, used, u64::MAX - 1, tag.as_deref()) {
                        if next != used {
                            used = next;
                            continue;
                        }
                    }
                }
                break;
            }
        }
    }
    Err(if last_error.is_empty() { "vault http request failed".to_string() } else { last_error })
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
/// `command` accepts a shell string or a string array (exact argv, no shell).
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

/// Live ping result: the key is exercised against its canonical endpoint and
/// its health is persisted, so the dashboard reflects it immediately.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultTestResult {
    pub success: bool,
    pub latency_ms: u64,
    pub message: String,
    pub health: String,
}

/// Blind `vault_test_connection`: ping the preset's verify endpoint with one
/// pooled key, persist the observed health, return latency + outcome. The
/// secret never appears in the result.
#[tauri::command]
pub async fn vault_test_connection(
    service: String,
    key_index: Option<usize>,
) -> Result<VaultTestResult, String> {
    let endpoint = {
        let paths = crate::paths::OrquestraPaths::resolve()?;
        let vault = load_vault(&paths)?;
        let current = vault
            .services
            .get(&service)
            .ok_or_else(|| format!("unknown vault service: {service}"))?;
        presets::verify_endpoint(&current.preset).ok_or_else(|| {
            format!("no canonical verify endpoint for the {} preset", current.preset)
        })?.to_string()
    };
    let paths = crate::paths::OrquestraPaths::resolve()?;
    let mut vault = load_vault(&paths)?;
    let snapshot = vault
        .services
        .get(&service)
        .ok_or_else(|| format!("unknown vault service: {service}"))?;
    if snapshot.keys.is_empty() {
        return Err(format!("vault service {service} has no keys"));
    }
    let used = key_index
        .unwrap_or(snapshot.active)
        .min(snapshot.keys.len().saturating_sub(1));
    let live = snapshot.clone();
    let secret = live.keys.get(used).map(|key| key.secret.clone()).unwrap_or_default();
    let outgoing = http::effective_headers(&live, &secret, &HashMap::new());
    let start = std::time::Instant::now();
    let outcome = http::execute_once("GET", &endpoint, &outgoing, None).await;
    let latency_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    match outcome {
        Ok((status, _, text)) => {
            let ok = (200..300).contains(&status);
            let stored = vault
                .services
                .get_mut(&service)
                .ok_or_else(|| format!("unknown vault service: {service}"))?;
            record_result(stored, used, status);
            let health = service_health(stored);
            let _ = save_vault(&paths, &vault);
            let scrubbed = http::scrub_text(&text, &live.keys.iter().map(|key| key.secret.clone()).collect::<Vec<_>>());
            let hint: String = scrubbed.chars().take(160).collect();
            Ok(VaultTestResult {
                success: ok,
                latency_ms,
                message: if ok {
                    format!("HTTP {status} in {latency_ms}ms")
                } else {
                    format!("HTTP {status} in {latency_ms}ms: {hint}")
                },
                health,
            })
        }
        Err(error) => Ok(VaultTestResult {
            success: false,
            latency_ms,
            message: error,
            health: service_health(&live),
        }),
    }
}

/// Remove one key from a pool (re-indexes, keeps a valid primary).
#[tauri::command]
pub fn vault_remove_key(id: String, index: usize) -> Result<VaultServicePublic, String> {
    let paths = crate::paths::OrquestraPaths::resolve()?;
    let mut vault = load_vault(&paths)?;
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
    let public = to_public(service);
    save_vault(&paths, &vault)?;
    Ok(public)
}
