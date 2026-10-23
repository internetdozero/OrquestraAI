//! Blind HTTP proxy: key injection, failover, health persistence.
//!
//! Split from `mod.rs` so each vault submodule stays under the repo's
//! file-size cap. The TOCTOU rule lives here: snapshots are short-lived and
//! never held across `.await`; every mutation reloads fresh under the
//! [`update_vault`](super::atomic::update_vault) lock.

use std::collections::HashMap;

use super::atomic::update_vault;
use super::http;
use super::presets;
use super::store::{
    first_key_index_in, health_for_status, load_vault, next_key_index_in, normalize_tag,
    record_result_in, service_health, StatusOutcome,
};

/// Methods that are safe to replay after a transport failure: repeating them
/// cannot double-apply a transaction. `POST` and `PATCH` are excluded —
/// without an idempotency key they return the error with no key rotation.
fn is_idempotent_method(verb: &str) -> bool {
    matches!(
        verb.to_ascii_uppercase().as_str(),
        "GET" | "HEAD" | "PUT" | "DELETE" | "OPTIONS"
    )
}

/// The caller opted into safe retry by sending an idempotency key.
fn has_idempotency_key(extra: &HashMap<String, String>) -> bool {
    extra.keys().any(|name| {
        name.eq_ignore_ascii_case("idempotency-key")
            || name.eq_ignore_ascii_case("x-idempotency-key")
    })
}

/// Transport-failure failover verdict: cycle to the next key only when the
/// request provably never reached the server, the method is idempotent, or
/// the caller sent an idempotency key. Pure so unit tests can pin it.
pub(super) fn should_retry_transport(
    verb: &str,
    extra: &HashMap<String, String>,
    safe_to_retry: bool,
) -> bool {
    safe_to_retry || is_idempotent_method(verb) || has_idempotency_key(extra)
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

/// Blind credentialed request: inject the active key, fail over on 401/402/
/// 403/429, strip auth response headers, scrub secrets from the body. The
/// shell's `vault_http_request` command wraps this.
pub async fn request(
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
    let verb = method.unwrap_or_else(|| "GET".to_string());
    let extra = headers.unwrap_or_default();
    let tag = key_tag.as_deref().and_then(normalize_tag);
    let paths = orquestra_core::paths::OrquestraPaths::resolve()?;
    // Short-lived snapshot for validation only — never held across `.await`.
    // Every mutation below reloads fresh under the vault lock, so a
    // concurrent `vault_save_service` is merged, not overwritten.
    let snapshot = load_vault(&paths)?
        .services
        .remove(&service)
        .ok_or_else(|| format!("unknown vault service: {service}"))?;
    // SSRF gate: the key rides in the headers, so the endpoint must be an
    // https URL on this preset's allowlisted hosts — attacker hosts,
    // plain http, IP literals and unknown presets die here, before any
    // secret is loaded into memory for the call. Supabase is locked to
    // the service's own project host via `service.meta`. The parsed `Url`
    // is what gets sent, so the gate and reqwest cannot disagree.
    let parsed = super::endpoints::validate_endpoint_for_service(
        &snapshot.preset,
        &endpoint,
        Some(&snapshot),
    )?;
    if !snapshot.enabled {
        return Err(format!("vault service {service} is disabled"));
    }
    if snapshot.keys.is_empty() {
        return Err(format!("vault service {service} has no keys"));
    }
    if let Some(wanted) = tag.as_deref() {
        let matches = snapshot
            .keys
            .iter()
            .filter(|key| {
                key.tag
                    .as_deref()
                    .map(|raw| raw.trim().to_lowercase())
                    .as_deref()
                    == Some(wanted)
            })
            .count();
        if matches == 0 {
            return Err(format!(
                "vault service {service} has no key tagged \"{wanted}\""
            ));
        }
    }
    let scoped_len = match tag.as_deref() {
        None => snapshot.keys.len(),
        Some(wanted) => snapshot
            .keys
            .iter()
            .filter(|key| {
                key.tag
                    .as_deref()
                    .map(|raw| raw.trim().to_lowercase())
                    .as_deref()
                    == Some(wanted)
            })
            .count()
            .max(1),
    };
    let attempts = scoped_len.max(1);
    let now_ms = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    };
    let start = snapshot.active.min(snapshot.keys.len().saturating_sub(1));
    let mut used =
        first_key_index_in(&snapshot.keys, start, now_ms(), tag.as_deref()).unwrap_or(start);
    let mut first_used = used;
    let mut last_error = String::new();
    for _ in 0..attempts {
        // Fresh read per attempt: pick the secret, then drop the vault long
        // before the seconds-long HTTP call.
        let (current, secret) = {
            let vault = load_vault(&paths)?;
            let current = vault
                .services
                .get(&service)
                .ok_or_else(|| format!("unknown vault service: {service}"))?
                .clone();
            let secret = current
                .keys
                .get(used)
                .map(|key| key.secret.clone())
                .ok_or_else(|| format!("vault service {service} has no key at {used}"))?;
            (current, secret)
        };
        let outgoing = http::effective_headers(&current, &secret, &extra);
        match http::execute_once(&verb, &parsed, &outgoing, body.as_deref()).await {
            Ok((status, headers, text)) => {
                let secrets: Vec<String> =
                    current.keys.iter().map(|key| key.secret.clone()).collect();
                let scrubbed = http::scrub_text(&text, &secrets);
                let rotated = used != first_used;
                // Reload-before-write: apply health to the LIVE vault,
                // matching the serving key by secret so a concurrent key
                // rotation cannot misattribute the outcome. Classification
                // runs on the scrubbed body, so no secret reaches the
                // 403 quota/permission logic or the persisted state.
                let new_active = update_vault(&paths, |vault| {
                    let live = vault
                        .services
                        .get_mut(&service)
                        .ok_or_else(|| format!("unknown vault service: {service}"))?;
                    let resolved = live
                        .keys
                        .iter()
                        .position(|key| key.secret == secret)
                        .or((used < live.keys.len()).then_some(used));
                    if let Some(index) = resolved {
                        used = index;
                        record_result_in(live, index, status, &scrubbed, tag.as_deref());
                        Ok(live.active)
                    } else {
                        Ok(used)
                    }
                })?;
                // Fail over transparently: retry once with the rotated key.
                // `new_active` is the just-persisted value, so no second
                // disk read is needed to observe our own write. Status-based
                // failover stays active for every method: the server rejected
                // the request before acting on it, so no replay hazard.
                if matches!(
                    health_for_status(status, &scrubbed),
                    StatusOutcome::Fault(_)
                ) && new_active != used
                {
                    used = new_active;
                    first_used = first_used.min(used);
                    continue;
                }
                return Ok(VaultHttpResult {
                    status,
                    headers,
                    body: scrubbed,
                    used_key_index: used,
                    rotated,
                });
            }
            Err(error) => {
                // Transport failure: cycle to the next key only when the
                // request provably never reached the server (connect
                // failure), the method is idempotent, or the caller sent an
                // idempotency key. Anything else returns the error with no
                // key rotation and no health change — replaying a `POST` or
                // `PATCH` after a read timeout could double-charge.
                if !should_retry_transport(&verb, &extra, error.safe_to_retry) {
                    return Err(error.message);
                }
                last_error = error.message;
                match load_vault(&paths) {
                    Ok(vault) => {
                        if let Some(live) = vault.services.get(&service) {
                            if let Some(next) =
                                next_key_index_in(&live.keys, used, u64::MAX - 1, tag.as_deref())
                            {
                                if next != used {
                                    used = next;
                                    continue;
                                }
                            }
                        }
                    }
                    // The HTTP error is already recorded below; a vault read
                    // failure here must not vanish too — log it and serve
                    // the recorded error instead of failing over blind.
                    Err(read_error) => {
                        crate::log_warn!("vault failover skipped for {service}: {read_error}");
                    }
                }
                break;
            }
        }
    }
    Err(if last_error.is_empty() {
        "vault http request failed".to_string()
    } else {
        last_error
    })
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

/// Ping the preset's verify endpoint with one pooled key, persist the
/// observed health, return latency + outcome. The secret never appears in the
/// result. The shell's `vault_test_connection` command wraps this.
pub async fn test_connection(
    service: String,
    key_index: Option<usize>,
) -> Result<VaultTestResult, String> {
    // The canonical ping goes through the same gate and sends the same
    // parsed `Url`, so a compromised preset table cannot redirect a test
    // ping (and its key) off-allowlist.
    let endpoint = {
        let paths = orquestra_core::paths::OrquestraPaths::resolve()?;
        let vault = load_vault(&paths)?;
        let current = vault
            .services
            .get(&service)
            .ok_or_else(|| format!("unknown vault service: {service}"))?;
        let raw = presets::verify_endpoint(&current.preset).ok_or_else(|| {
            format!(
                "no canonical verify endpoint for the {} preset",
                current.preset
            )
        })?;
        super::endpoints::validate_endpoint_for_service(&current.preset, raw, Some(current))?
    };
    let paths = orquestra_core::paths::OrquestraPaths::resolve()?;
    // Snapshot released before the network call; health is applied to the
    // live vault afterwards so concurrent writers are never overwritten.
    let (live, used, secret) = {
        let vault = load_vault(&paths)?;
        let snapshot = vault
            .services
            .get(&service)
            .ok_or_else(|| format!("unknown vault service: {service}"))?;
        if !snapshot.enabled {
            return Err(format!("vault service {service} is disabled"));
        }
        if snapshot.keys.is_empty() {
            return Err(format!("vault service {service} has no keys"));
        }
        let used = key_index
            .unwrap_or(snapshot.active)
            .min(snapshot.keys.len().saturating_sub(1));
        let live = snapshot.clone();
        let secret = live
            .keys
            .get(used)
            .map(|key| key.secret.clone())
            .unwrap_or_default();
        (live, used, secret)
    };
    let outgoing = http::effective_headers(&live, &secret, &HashMap::new());
    let start = std::time::Instant::now();
    let outcome = http::execute_once("GET", &endpoint, &outgoing, None).await;
    let latency_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    match outcome {
        Ok((status, _, text)) => {
            let ok = (200..300).contains(&status);
            let scrubbed = http::scrub_text(
                &text,
                &live
                    .keys
                    .iter()
                    .map(|key| key.secret.clone())
                    .collect::<Vec<_>>(),
            );
            let health = update_vault(&paths, |vault| {
                let stored = vault
                    .services
                    .get_mut(&service)
                    .ok_or_else(|| format!("unknown vault service: {service}"))?;
                let resolved = stored
                    .keys
                    .iter()
                    .position(|key| key.secret == secret)
                    .or((used < stored.keys.len()).then_some(used));
                if let Some(index) = resolved {
                    record_result_in(stored, index, status, &scrubbed, None);
                }
                Ok(service_health(stored))
            })?;
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
            message: error.message,
            health: service_health(&live),
        }),
    }
}
