//! Encrypted-at-rest vault storage + multi-key rotation.
//!
//! Secrets live in `~/.orquestra/vault.json` sealed with AES-256-GCM under
//! a per-install file key (OS keyring, `0600` key-file fallback on headless
//! hosts), written atomically via tmp+rename with `0600` as defense in
//! depth, and never crossing IPC in plaintext. `list` returns masked
//! previews only. See `crate::at_rest` for the envelope and migration.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use orquestra_core::paths::OrquestraPaths;

const VAULT_FILE: &str = "vault.json";
const COOLDOWN_SECS: u64 = 300;
/// A transient 401 cools the key down instead of disabling it forever: the
/// key stays out of rotation for 10 minutes, then becomes eligible again so
/// a later success can heal it (or another 401 can re-cool it).
const INVALID_COOLDOWN_SECS: u64 = 600;

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
    let content = match fs::read_to_string(&path) {
        Ok(content) if content.trim().is_empty() => return Ok(VaultFile::default()),
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(VaultFile::default()),
        Err(error) => {
            return Err(format!("failed to read {}: {error}", path.display()));
        }
    };
    // The at-rest layer owns legacy migration (backup + verify + unlink),
    // v1 → v2 upgrades, keyring promotion, and backup self-healing — the
    // loader just parses the returned plaintext.
    let (plaintext, _) =
        crate::at_rest::open_stored(paths, &path, &content, crate::at_rest::AAD_VAULT)
            .map_err(|error| format!("failed to unseal {}: {error}", path.display()))?;
    let vault: VaultFile = serde_json::from_str(&plaintext)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    Ok(vault)
}

/// Zeroize and unlink the vault's `.migrate-bak` plaintext backup. The
/// sealed vault must still open under the current key, so this never
/// destroys the only readable copy; see `crate::at_rest`.
pub(super) fn purge_migration_backup(paths: &OrquestraPaths) -> Result<bool, String> {
    crate::at_rest::purge_migration_backup(paths, &vault_path(paths), crate::at_rest::AAD_VAULT)
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
    let sealed = crate::at_rest::seal_text(paths, &content, crate::at_rest::AAD_VAULT)?;
    super::atomic::write_private(&path, &sealed)
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
        // An `InvalidKey` still in its cooldown stays out of rotation; once
        // the cooldown lapses it becomes eligible again so a later success
        // can heal it (transient 401s never disable a key forever).
        if key.health == KeyHealth::InvalidKey && key.cooldown_until_ms > now {
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
        if key.health == KeyHealth::InvalidKey && key.cooldown_until_ms > now {
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

/// What one HTTP status means for the serving key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusOutcome {
    /// Success (or any non-fault status): heal the key.
    Healthy,
    /// A key fault: cool the key down and fail over.
    Fault(KeyHealth),
    /// Decidedly not a key fault (a 403 permission denial): no health
    /// change, no cooldown, no failover — the status is returned as is.
    Neutral,
}

/// Body markers that identify a 403 as quota/billing exhaustion rather than
/// a permission denial. Matched case-insensitively against the scrubbed body.
fn is_quota_body(body: &str) -> bool {
    let lowered = body.to_ascii_lowercase();
    ["quota", "billing", "insufficient", "exceeded", "limit"]
        .iter()
        .any(|marker| lowered.contains(marker))
}

/// Classify an HTTP status into key health. `body` (already scrubbed)
/// separates 403 quota exhaustion from 403 permission denials.
pub fn health_for_status(status: u16, body: &str) -> StatusOutcome {
    match status {
        429 => StatusOutcome::Fault(KeyHealth::RateLimited),
        402 => StatusOutcome::Fault(KeyHealth::QuotaExceeded),
        401 => StatusOutcome::Fault(KeyHealth::InvalidKey),
        403 if is_quota_body(body) => StatusOutcome::Fault(KeyHealth::QuotaExceeded),
        403 => StatusOutcome::Neutral,
        _ => StatusOutcome::Healthy,
    }
}

/// Record one request outcome: cooldown the failing key, fail over.
/// Untagged shorthand over [`record_result_in`]; only unit tests call this
/// directly now (production paths are tag-scoped). Kept — not deleted — so
/// the rotation semantics stay covered without duplicating the test.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn record_result(service: &mut VaultService, used: usize, status: u16) {
    record_result_in(service, used, status, "", None)
}

/// Tag-scoped variant: failover stays inside the requested tag. `body` is
/// the scrubbed response body, used only to separate 403 quota exhaustion
/// from 403 permission denials.
pub(super) fn record_result_in(
    service: &mut VaultService,
    used: usize,
    status: u16,
    body: &str,
    tag: Option<&str>,
) {
    // Healthy and neutral outcomes need no work after their match arms.
    match health_for_status(status, body) {
        StatusOutcome::Healthy => {
            if let Some(key) = service.keys.get_mut(used) {
                key.health = KeyHealth::Healthy;
                key.failures = 0;
                key.cooldown_until_ms = 0;
            }
        }
        StatusOutcome::Neutral => (),
        StatusOutcome::Fault(fault) => {
            let now = now_ms();
            // Only the serving key is ever touched: healthy reserves are
            // never disturbed by one key's outcome.
            if let Some(key) = service.keys.get_mut(used) {
                key.health = fault;
                key.failures = key.failures.saturating_add(1);
                let window = if fault == KeyHealth::InvalidKey {
                    INVALID_COOLDOWN_SECS
                } else {
                    COOLDOWN_SECS
                };
                key.cooldown_until_ms = now.saturating_add(window.saturating_mul(1000));
            }
            if let Some(next) = next_key_index_in(&service.keys, used, now, tag) {
                service.active = next;
            }
            service.updated_at_ms = now;
        }
    }
}
