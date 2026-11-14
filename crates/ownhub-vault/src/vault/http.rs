//! Blind HTTP execution: inject the active key in memory, strip secrets back.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use futures_util::StreamExt;

use super::presets::build_headers;
use super::store::VaultService;

/// Cap on any single response body: without it a hostile endpoint can grow
/// the IPC payload (and this process's memory) without bound.
const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

/// One process-wide client, so every `vault_http_request` reuses the same
/// TCP/TLS connection pool and session resumption instead of paying a fresh
/// handshake per call. Built once; the redirect fence below is baked in.
static SHARED_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub(super) fn shared_client() -> Result<&'static reqwest::Client, String> {
    if let Some(client) = SHARED_CLIENT.get() {
        return Ok(client);
    }
    // Redirects stay on the first request's exact origin — same scheme,
    // same port, same host — and stop after 5 hops. The initial gate only
    // admits https/443, so chaining the fence to the first URL keeps the
    // whole call on that origin: downgrades, port pivots and cross-host
    // bounces stop (the 302 is returned, never followed with headers).
    // reqwest's default policy is same-host but scheme- and port-blind
    // with a 10-hop limit; the custom fence closes those gaps.
    let fenced = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 5 {
            return attempt.stop();
        }
        let Some(first) = attempt.previous().first() else {
            return attempt.stop();
        };
        let current = attempt.url();
        let host = current.host_str().unwrap_or("").to_ascii_lowercase();
        let same_origin = !host.is_empty()
            && current.scheme() == first.scheme()
            && current.port_or_known_default() == first.port_or_known_default()
            && host == first.host_str().unwrap_or("").to_ascii_lowercase();
        if same_origin {
            attempt.follow()
        } else {
            attempt.stop()
        }
    });
    // Built once; a losing racer's client is dropped and the winner kept.
    let built = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(fenced)
        .build()
        .map_err(|error| format!("vault http client failed: {error}"))?;
    let _ = SHARED_CLIENT.set(built);
    SHARED_CLIENT
        .get()
        .ok_or_else(|| "vault http client failed to init".to_string())
}

/// Read the body in chunks, stopping past the cap. The `content-length`
/// pre-check kills the declared-huge case before a byte is read; the
/// running total covers liars and chunked encodings with no length.
async fn read_capped_body(response: reqwest::Response) -> Result<String, String> {
    if response
        .content_length()
        .is_some_and(|len| len > MAX_BODY_BYTES as u64)
    {
        return Err(format!(
            "vault http body exceeds the {} byte cap",
            MAX_BODY_BYTES
        ));
    }
    let mut buffered: usize = 0;
    let mut out = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("vault http body failed: {error}"))?;
        buffered = buffered.saturating_add(chunk.len());
        if buffered > MAX_BODY_BYTES {
            return Err(format!(
                "vault http body exceeds the {} byte cap",
                MAX_BODY_BYTES
            ));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

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

/// Transport failure with a retry verdict. A connect failure means nothing
/// was sent, so cycling to the next key is safe. A timeout, body error or
/// any failure after the request went out may have reached the server —
/// retrying a non-idempotent call there could double-apply a transaction —
/// so the caller only fails over for idempotent methods (or callers that
/// sent an idempotency key) and status-based rejections.
#[derive(Debug, Clone)]
pub(super) struct TransportError {
    pub message: String,
    pub safe_to_retry: bool,
}

impl From<String> for TransportError {
    /// Body/read failures happen after the request went out.
    fn from(message: String) -> Self {
        Self {
            message,
            safe_to_retry: false,
        }
    }
}

/// Execute one HTTP call with `reqwest`, returning status + filtered output.
/// Failover/rotation is owned by the caller (`mod.rs`), which retries with
/// the next key on 401/402/403/429.
/// Execute one validated call. `url` is the `Url` the endpoint gate
/// returned — never the raw caller string — so the sender and the
/// validator agree on scheme, host and port by construction.
pub(super) async fn execute_once(
    method: &str,
    url: &url::Url,
    headers: &[(String, String)],
    body: Option<&str>,
) -> Result<(u16, HashMap<String, String>, String), TransportError> {
    let client = shared_client()?;
    let verb = method.to_ascii_uppercase();
    let mut request = match verb.as_str() {
        "GET" => client.get(url.clone()),
        "POST" => client.post(url.clone()),
        "PUT" => client.put(url.clone()),
        "PATCH" => client.patch(url.clone()),
        "DELETE" => client.delete(url.clone()),
        "HEAD" => client.head(url.clone()),
        _ => {
            return Err(TransportError {
                // Deterministic rejection, never sent: retrying another key
                // would fail identically, so report immediately.
                safe_to_retry: false,
                message: format!("unsupported vault http method: {method}"),
            });
        }
    };
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }
    if let Some(payload) = body {
        request = request.body(payload.to_string());
    }
    let response = request.send().await.map_err(|error| TransportError {
        // `is_connect` (refused, DNS, no route) means nothing left this
        // process. Timeouts, body errors and everything later may have
        // reached the server — unsafe to replay without idempotency.
        safe_to_retry: error.is_connect(),
        message: format!("vault http request failed: {error}"),
    })?;
    let status = response.status().as_u16();
    let mut raw: HashMap<String, String> = HashMap::new();
    for (name, value) in response.headers().iter() {
        raw.insert(name.to_string(), value.to_str().unwrap_or("").to_string());
    }
    let text = read_capped_body(response).await?;
    Ok((status, strip_response_headers(&raw), text))
}
