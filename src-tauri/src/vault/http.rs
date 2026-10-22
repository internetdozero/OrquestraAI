//! Blind HTTP execution: inject the active key in memory, strip secrets back.

use std::collections::{HashMap, HashSet};

use super::presets::build_headers;
use super::store::VaultService;

/// Headers never returned to the agent (auth material + tracking).
fn sensitive_response_headers() -> HashSet<String> {
    [
        "authorization",
        "proxy-authenticate",
        "proxy-authorization",
        "cookie",
        "set-cookie",
        "x-api-key",
        "x-goog-api-key",
        "api-key",
        "apikey",
    ]
    .iter()
    .map(|header| header.to_string())
    .collect()
}

/// Remove auth / cookie headers from a response header map (case-insensitive).
pub fn strip_response_headers(headers: &HashMap<String, String>) -> HashMap<String, String> {
    let blocked = sensitive_response_headers();
    headers
        .iter()
        .filter(|(name, _)| !blocked.contains(&name.to_ascii_lowercase()))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// Merge caller headers with blind auth headers. Caller `authorization`
/// values are ignored so the vault stays authoritative; the raw key only
/// exists inside the returned map, never in agent context.
pub fn effective_headers(
    service: &VaultService,
    secret: &str,
    extra: &HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut merged: HashMap<String, String> = HashMap::new();
    for (name, value) in extra {
        if name.eq_ignore_ascii_case("authorization")
            || name.eq_ignore_ascii_case("x-api-key")
            || name.eq_ignore_ascii_case("x-goog-api-key")
        {
            continue;
        }
        merged.insert(name.clone(), value.clone());
    }
    for (name, value) in build_headers(
        &service.preset,
        secret,
        service.custom_header.as_deref(),
        service.custom_prefix.as_deref(),
        service.custom_env.as_deref(),
    ) {
        merged.insert(name, value);
    }
    merged.into_iter().collect()
}

/// Scrub every known secret from a response body before returning it.
///
/// Encoding-aware: besides the raw key, base64 (standard + URL-safe,
/// padded + unpadded), hex, URL-encoded and reversed forms are masked, so
/// a transformed echo cannot smuggle the key past the filter. See
/// [`super::scrub`] for the variant table and its tests.
pub fn scrub_body(body: &str, secrets: &[&str]) -> String {
    super::scrub::scrub_with_variants(body, secrets)
}

/// Scrub secrets from free-form text (shared with `exec` output).
pub fn scrub_text(text: &str, secrets: &[String]) -> String {
    let refs: Vec<&str> = secrets.iter().map(String::as_str).collect();
    scrub_body(text, &refs)
}

/// Execute one HTTP call with `reqwest`, returning status + filtered output.
/// Failover/rotation is owned by the caller (`mod.rs`), which retries with
/// the next key on 401/402/403/429.
pub(super) async fn execute_once(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: Option<&str>,
) -> Result<(u16, HashMap<String, String>, String), String> {
    // Same-host redirects only: a compromised API must not bounce the call
    // — and its auth headers — to an attacker host. reqwest already strips
    // `Authorization` on cross-host hops; refusing them outright removes the
    // SSRF-via-redirect class entirely.
    let same_host_only = reqwest::redirect::Policy::custom(|attempt| {
        let current = attempt.url().host_str().unwrap_or("").to_ascii_lowercase();
        let first = attempt
            .previous()
            .first()
            .and_then(|url| url.host_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !current.is_empty() && current == first {
            attempt.follow()
        } else {
            attempt.stop()
        }
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(same_host_only)
        .build()
        .map_err(|error| format!("vault http client failed: {error}"))?;
    let verb = method.to_ascii_uppercase();
    let mut request = match verb.as_str() {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "PATCH" => client.patch(url),
        "DELETE" => client.delete(url),
        "HEAD" => client.head(url),
        _ => return Err(format!("unsupported vault http method: {method}")),
    };
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }
    if let Some(payload) = body {
        request = request.body(payload.to_string());
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("vault http request failed: {error}"))?;
    let status = response.status().as_u16();
    let mut raw: HashMap<String, String> = HashMap::new();
    for (name, value) in response.headers().iter() {
        raw.insert(name.to_string(), value.to_str().unwrap_or("").to_string());
    }
    let text = response
        .text()
        .await
        .map_err(|error| format!("vault http body failed: {error}"))?;
    Ok((status, strip_response_headers(&raw), text))
}
