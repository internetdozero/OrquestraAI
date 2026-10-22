//! Encrypted-at-rest-by-permissions vault storage + multi-key rotation.
//!
//! Design decision: secrets live in `~/.orquestra/vault.json` with `0600`
//! (same pattern as `mcps.json`), written atomically via tmp+rename and
//! never crossing IPC in plaintext. `list` returns masked previews only.
//! OS keyring / AES-GCM would need an interactive unlock on headless hosts,
//! so file permissions + in-memory-only injection is the pragmatic default.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::OrquestraPaths;

const VAULT_FILE: &str = "vault.json";
const COOLDOWN_SECS: u64 = 300;

/// Per-key circuit-breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum KeyHealth {
    #[default]
    Healthy,
    RateLimited,
    QuotaExceeded,
    InvalidKey,
}

/// One stored key (secret never leaves the backend unmasked).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultKey {
    pub id: String,
    pub secret: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub health: KeyHealth,
    #[serde(default)]
    pub cooldown_until_ms: u64,
    #[serde(default)]
    pub failures: u32,
}

/// One service entry: 1 primary + N reserve keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultService {
    pub id: String,
    pub label: String,
    #[serde(default = "default_preset")]
    pub preset: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub keys: Vec<VaultKey>,
    #[serde(default)]
    pub active: usize,
    #[serde(default)]
    pub custom_header: Option<String>,
    #[serde(default)]
    pub custom_prefix: Option<String>,
    #[serde(default)]
    pub custom_env: Option<String>,
    /// Non-secret companion values (account ids, regions, project URLs).
    #[serde(default)]
    pub meta: HashMap<String, String>,
    #[serde(default)]
    pub updated_at_ms: u64,
}

fn default_preset() -> String {
    "custom".to_string()
}

/// Masked public view: no plaintext secrets cross IPC.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultServicePublic {
    pub id: String,
    pub label: String,
    pub preset: String,
    pub description: String,
    pub active_key_index: usize,
    pub total_keys: usize,
    pub masked_keys: Vec<String>,
    pub health: String,
    pub key_health: Vec<KeyHealth>,
    /// Optional per-key tag (e.g. "workers", "d1"). Parallel to `masked_keys`.
    pub key_tags: Vec<Option<String>>,
    /// Custom auth shaping (header/prefix/env names only — never secrets).
    #[serde(default)]
    pub custom_header: Option<String>,
    #[serde(default)]
    pub custom_prefix: Option<String>,
    #[serde(default)]
    pub custom_env: Option<String>,
    /// Cooldown expiry per key (epoch ms, 0 = none). Drives countdown badges.
    pub cooldown_until_ms: Vec<u64>,
    /// Non-secret companion values, safe to cross IPC.
    pub meta: HashMap<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VaultFile {
    #[serde(default)]
    pub services: BTreeMap<String, VaultService>,
}

pub(super) fn vault_path(paths: &OrquestraPaths) -> PathBuf {
    paths.root.join(VAULT_FILE)
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn mask_key(secret: &str) -> String {
    if secret.is_empty() {
        return String::new();
    }
    if secret.len() <= 8 {
        return "********".to_string();
    }
    let prefix_len = 6.min(secret.len());
    let prefix: String = secret.chars().take(prefix_len).collect();
    let suffix: String = secret
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{prefix}...{suffix}")
}

pub(super) fn service_health(service: &VaultService) -> String {
    if service.keys.is_empty() {
        return "Empty".to_string();
    }
    let active_health = service
        .keys
        .get(service.active)
        .map(|key| key.health)
        .unwrap_or(KeyHealth::Healthy);
    match active_health {
        KeyHealth::Healthy => {
            if service
                .keys
                .iter()
                .any(|key| key.health != KeyHealth::Healthy)
            {
                "Degraded".to_string()
            } else {
                "Healthy".to_string()
            }
        }
        KeyHealth::RateLimited => "RateLimited".to_string(),
        KeyHealth::QuotaExceeded => "QuotaExceeded".to_string(),
        KeyHealth::InvalidKey => "InvalidKey".to_string(),
    }
}

pub(super) fn to_public(service: &VaultService) -> VaultServicePublic {
    VaultServicePublic {
        id: service.id.clone(),
        label: service.label.clone(),
        preset: service.preset.clone(),
        description: service.description.clone(),
        active_key_index: service.active,
        total_keys: service.keys.len(),
        masked_keys: service
            .keys
            .iter()
            .map(|key| mask_key(&key.secret))
            .collect(),
        health: service_health(service),
        key_health: service.keys.iter().map(|key| key.health).collect(),
        key_tags: service
            .keys
            .iter()
            .map(|key| normalized_tag(key.tag.clone()))
            .collect(),
        custom_header: service.custom_header.clone(),
        custom_prefix: service.custom_prefix.clone(),
        custom_env: service.custom_env.clone(),
        cooldown_until_ms: service
            .keys
            .iter()
            .map(|key| key.cooldown_until_ms)
            .collect(),
        meta: service.meta.clone(),
    }
}

pub(super) fn load_vault(paths: &OrquestraPaths) -> Result<VaultFile, String> {
    let path = vault_path(paths);
    if fs::symlink_metadata(&path)
        .ok()
        .is_some_and(|meta| meta.file_type().is_symlink())
    {
        return Err(format!(
            "refusing to read {}: not a regular file",
            path.display()
        ));
    }
    match fs::read_to_string(&path) {
        Ok(content) if content.trim().is_empty() => Ok(VaultFile::default()),
        Ok(content) => serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse {}: {error}", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(VaultFile::default()),
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}

pub(super) fn save_vault(paths: &OrquestraPaths, vault: &VaultFile) -> Result<(), String> {
    let path = vault_path(paths);
    if fs::symlink_metadata(&path)
        .ok()
        .is_some_and(|meta| meta.file_type().is_symlink())
    {
        return Err(format!(
            "refusing to replace {}: not a regular file",
            path.display()
        ));
    }
    let mut content = serde_json::to_string_pretty(vault)
        .map_err(|error| format!("failed to serialize the credential vault: {error}"))?;
    content.push('\n');
    super::atomic::write_private(&path, &content)
}

pub(super) fn validate_id(id: &str) -> Result<(), String> {
    if id.trim().is_empty()
        || id == "."
        || id == ".."
        || id.starts_with('.')
        || id
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '/' | '\\' | '"' | '\'' | '\0'))
    {
        return Err(format!("invalid vault service id: {id:?}"));
    }
    Ok(())
}

/// Normalize a stored tag: trim, drop empties, lowercase for matching.
pub fn normalize_tag(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_lowercase())
}

/// Resolve the stored tag for one key being saved.
/// - `Some(entry)` (the caller sent a tag array covering this key): wins;
///   `None`/blank inside the array clears the tag.
/// - `None` (no array, or a short array): keep the previous tag, so old
///   callers round-tripping masks never wipe tags by accident.
pub fn resolve_saved_tag(
    explicit: Option<&Option<String>>,
    prev_tag: Option<String>,
) -> Option<String> {
    match explicit {
        Some(inner) => inner.as_deref().and_then(normalize_tag),
        None => prev_tag,
    }
}

fn normalized_tag(tag: Option<String>) -> Option<String> {
    tag.as_deref().and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

/// Next healthy key index after `failed`, skipping cooldown + invalid keys.
#[allow(dead_code)]
pub fn next_key_index(keys: &[VaultKey], failed: usize, now: u64) -> Option<usize> {
    next_key_index_in(keys, failed, now, None)
}

/// Tag-scoped failover: only keys whose normalized tag matches `tag`.
/// `None` keeps the legacy behavior (whole pool).
pub fn next_key_index_in(
    keys: &[VaultKey],
    failed: usize,
    now: u64,
    tag: Option<&str>,
) -> Option<usize> {
    let wanted = tag.and_then(normalize_tag);
    if keys.len() <= 1 {
        if keys.len() == 1 && matches_tag(&keys[0], wanted.as_deref()) {
            return None;
        }
        if keys.is_empty() {
            return None;
        }
    }
    for step in 1..keys.len().max(2) {
        if step >= keys.len() {
            break;
        }
        let candidate = (failed + step) % keys.len();
        let key = &keys[candidate];
        if !matches_tag(key, wanted.as_deref()) {
            continue;
        }
        if key.health == KeyHealth::InvalidKey {
            continue;
        }
        if key.cooldown_until_ms > now {
            continue;
        }
        return Some(candidate);
    }
    None
}

fn matches_tag(key: &VaultKey, wanted: Option<&str>) -> bool {
    match wanted {
        None => true,
        Some(tag) => {
            key.tag
                .as_deref()
                .map(|raw| raw.trim().to_lowercase())
                .as_deref()
                == Some(tag)
        }
    }
}

/// First healthy key index inside an optional tag scope.
pub fn first_key_index_in(
    keys: &[VaultKey],
    start: usize,
    now: u64,
    tag: Option<&str>,
) -> Option<usize> {
    let wanted = tag.and_then(normalize_tag);
    if keys.is_empty() {
        return None;
    }
    for step in 0..keys.len() {
        let candidate = (start + step) % keys.len();
        let key = &keys[candidate];
        if !matches_tag(key, wanted.as_deref()) {
            continue;
        }
        if key.health == KeyHealth::InvalidKey {
            continue;
        }
        if key.cooldown_until_ms > now {
            continue;
        }
        return Some(candidate);
    }
    // No healthy candidate: fall back to the first tag match so the caller
    // can surface the upstream status instead of a local "no key" error.
    keys.iter()
        .position(|key| matches_tag(key, wanted.as_deref()))
}

/// Classify an HTTP status into key health. `None` means no key fault.
pub fn health_for_status(status: u16) -> Option<KeyHealth> {
    match status {
        429 => Some(KeyHealth::RateLimited),
        402 | 403 => Some(KeyHealth::QuotaExceeded),
        401 => Some(KeyHealth::InvalidKey),
        _ => None,
    }
}

/// Record one request outcome: cooldown the failing key, fail over.
/// Untagged shorthand over [`record_result_in`]; only unit tests call this
/// directly now (production paths are tag-scoped). Kept — not deleted — so
/// the rotation semantics stay covered without duplicating the test.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn record_result(service: &mut VaultService, used: usize, status: u16) {
    record_result_in(service, used, status, None)
}

/// Tag-scoped variant: failover stays inside the requested tag.
pub(super) fn record_result_in(
    service: &mut VaultService,
    used: usize,
    status: u16,
    tag: Option<&str>,
) {
    let Some(fault) = health_for_status(status) else {
        if let Some(key) = service.keys.get_mut(used) {
            key.health = KeyHealth::Healthy;
            key.failures = 0;
            key.cooldown_until_ms = 0;
        }
        return;
    };
    let now = now_ms();
    if let Some(key) = service.keys.get_mut(used) {
        key.health = fault;
        key.failures = key.failures.saturating_add(1);
        if fault != KeyHealth::InvalidKey {
            key.cooldown_until_ms = now.saturating_add(COOLDOWN_SECS.saturating_mul(1000));
        }
    }
    if let Some(next) = next_key_index_in(&service.keys, used, now, tag) {
        service.active = next;
    }
    service.updated_at_ms = now;
}
