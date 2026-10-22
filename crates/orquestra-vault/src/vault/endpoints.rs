//! SSRF-proof endpoint gate for `vault_http_request`.
//!
//! The vault key is injected into request headers, so an unrestricted URL
//! lets any IPC caller exfiltrate keys to an attacker host, or pivot into
//! the local network (RFC 1918, link-local metadata services). Every
//! endpoint must therefore be `https://` on an allowlisted host for the
//! service's preset — exact host or `*.suffix` wildcard — with IP literals,
//! userinfo and non-443 ports rejected outright.
//!
//! The gate parses with the same `url` crate `reqwest` sends with, and the
//! parsed `Url` is what gets sent (see `proxy.rs` / `http.rs`), so the
//! validator and the sender cannot disagree on the host — that disagreement
//! is exactly the backslash (`evil.example\.supabase.co`) bypass class.
//!
//! Two residual notes, by design: (1) DNS resolution is left to the TLS
//! stack — a rebinding that points an allowlisted name at link-local fails
//! certificate verification in `reqwest`/`rustls`, which we do not disable;
//! (2) redirects are fenced to same scheme, same port and same host in
//! `http::shared_client`, so a compromised API cannot bounce the call (and
//! its headers) elsewhere, and stop after 5 hops.

use url::{Host, Url};

use super::store::VaultService;

/// Hosts a preset may call. `*.` entries match one-or-more left labels
/// (`xyz.supabase.co` matches `*.supabase.co`); anything else is exact.
/// Supabase is absent on purpose: its host is per-project, derived from the
/// service `meta` (see `supabase_host`).
pub fn allowed_hosts(preset_id: &str) -> &'static [&'static str] {
    match preset_id {
        "openai" => &["api.openai.com"],
        "anthropic" => &["api.anthropic.com"],
        "gemini" => &["generativelanguage.googleapis.com"],
        "openrouter" => &["openrouter.ai"],
        "groq" => &["api.groq.com"],
        "mistral" => &["api.mistral.ai"],
        "deepseek" => &["api.deepseek.com"],
        "perplexity" => &["api.perplexity.ai"],
        "together" => &["api.together.xyz"],
        "cohere" => &["api.cohere.com"],
        "elevenlabs" => &["api.elevenlabs.com"],
        "cloudflare" => &["api.cloudflare.com"],
        "github" => &["api.github.com"],
        "gitlab" => &["gitlab.com"],
        "stripe" => &["api.stripe.com"],
        "resend" => &["api.resend.com"],
        "tavily" => &["api.tavily.com"],
        "deepgram" => &["api.deepgram.com"],
        "unsplash" => &["api.unsplash.com"],
        "pexels" => &["api.pexels.com"],
        "pixabay" => &["pixabay.com"],
        "exa" => &["api.exa.ai"],
        "vercel" => &["api.vercel.com"],
        "neon" => &["console.neon.tech"],
        // No bearer-over-HTTP shape: aws (SigV4), bluesky/jules/pix
        // (per-account hosts or session flows) and custom (unknown).
        // Those keys stay usable via vault_exec; HTTP is refused with a
        // message rather than an open gate.
        // No static entry either: supabase is decided per service from
        // `service.meta` in `validate_endpoint_for_service`.
        _ => &[],
    }
}

/// Reject anything that is not an `https://` URL on the preset's allowlist,
/// returning the parsed `Url` the sender must use verbatim. Fails closed on
/// any parse error or constraint violation.
///
/// Non-Supabase shorthand for [`validate_endpoint_for_service`]; kept (not
/// inlined at the two [`validate_endpoint_for_service`] call sites) so the
/// preset-only contract stays covered without a `None` at every call.
#[cfg_attr(not(test), allow(dead_code))]
pub fn validate_endpoint(preset_id: &str, endpoint: &str) -> Result<Url, String> {
    validate_endpoint_for_service(preset_id, endpoint, None)
}

/// Supabase-aware gate: the allowed host is the service's own project host,
/// derived from `service.meta` — never another project's `*.supabase.co`.
/// Every other preset ignores `service` and behaves like
/// [`validate_endpoint`]. Returns the parsed `Url` the sender must use.
pub fn validate_endpoint_for_service(
    preset_id: &str,
    endpoint: &str,
    service: Option<&VaultService>,
) -> Result<Url, String> {
    let url = parse_https_url(endpoint)?;
    if preset_id == "supabase" {
        let Some(service) = service else {
            return Err(supabase_meta_guidance());
        };
        let expected = supabase_host(service)?;
        let host = url.host_str().unwrap_or("").to_ascii_lowercase();
        if host != expected {
            return Err(format!(
                "vault endpoint host {host:?} is not this service's Supabase project host ({expected:?})"
            ));
        }
        return Ok(url);
    }
    let allowed = allowed_hosts(preset_id);
    if allowed.is_empty() {
        return Err(format!(
            "the {preset_id} preset has no allowlisted HTTP hosts: use vault_exec for this service"
        ));
    }
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    let permitted = allowed.iter().any(|entry| {
        if let Some(suffix) = entry.strip_prefix("*.") {
            host.len() > suffix.len()
                && host.ends_with(suffix)
                && host.as_bytes()[host.len() - suffix.len() - 1] == b'.'
        } else {
            host == *entry
        }
    });
    if !permitted {
        return Err(format!(
            "vault endpoint host {host:?} is not allowlisted for the {preset_id} preset"
        ));
    }
    Ok(url)
}

/// Parse and harden one endpoint. Raw backslashes, tabs, newlines, other
/// control characters, and encoded separators (`%5c`, `%2f`) in the
/// authority are rejected before parsing: `url::Url` (like browsers) treats
/// `\` as a `/` delimiter, so `https://evil.example\.supabase.co/x`
/// otherwise parses with host `evil.example` and a harmless-looking path
/// while a naive sender could still reach the attacker host. After parsing:
/// `https`, port 443, no userinfo, and a domain host (rejects every IP
/// notation — `Host::Domain` never holds IPv4/IPv6, decimal, hex or octal).
fn parse_https_url(endpoint: &str) -> Result<Url, String> {
    reject_confusable_authority(endpoint)?;
    let url = Url::parse(endpoint).map_err(|_| "vault endpoint is not a valid URL".to_string())?;
    if url.scheme() != "https" {
        return Err("vault endpoint must use https:// (plain http is refused)".to_string());
    }
    if url.port_or_known_default() != Some(443) {
        return Err("vault endpoint port is not allowed (443 only)".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("vault endpoint must not contain userinfo".to_string());
    }
    match url.host() {
        Some(Host::Domain(_)) => Ok(url),
        _ => Err(
            "vault endpoint host is an IP literal or missing: only provider hostnames are allowed"
                .to_string(),
        ),
    }
}

/// Pre-parse scan of the raw authority. `url::Url` applies WHATWG
/// special-scheme rules: on `https://`, `\`, tab and newline are treated as
/// `/`, so the authority the parser sees can differ from the one a manual
/// split (or a different sender) would derive. Rejecting the raw bytes
/// first keeps the validator and `reqwest` — which shares this same crate —
/// in agreement.
fn reject_confusable_authority(endpoint: &str) -> Result<(), String> {
    let rest = endpoint
        .strip_prefix("https://")
        .ok_or_else(|| "vault endpoint must use https:// (plain http is refused)".to_string())?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err("vault endpoint has no host".to_string());
    }
    if authority.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err("vault endpoint authority contains a control character".to_string());
    }
    if authority.contains(['\\', '\t', '\n', '\r']) {
        return Err("vault endpoint authority contains a confusable separator".to_string());
    }
    let lowered = authority.to_ascii_lowercase();
    if lowered.contains("%5c") || lowered.contains("%2f") {
        return Err("vault endpoint authority contains an encoded separator".to_string());
    }
    if authority.contains('@') {
        return Err("vault endpoint must not contain userinfo".to_string());
    }
    Ok(())
}

/// Derive the service's own Supabase project host from `service.meta`:
/// `project_ref` (`^[a-z0-9]{20}$`) gives `<ref>.supabase.co`; otherwise
/// `endpoint` (the stored project URL, e.g. `SUPABASE_URL`), parsed with the
/// same rules, whose host must be exactly one label under `.supabase.co`.
/// Anything else — including a bare `https://supabase.co` — fails closed.
fn supabase_host(service: &VaultService) -> Result<String, String> {
    if let Some(project_ref) = service.meta.get("project_ref").map(|s| s.trim()) {
        if !project_ref.is_empty() {
            if project_ref.len() == 20
                && project_ref
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            {
                return Ok(format!("{project_ref}.supabase.co"));
            }
            return Err(
                "supabase project_ref must be 20 lowercase alphanumeric characters".to_string(),
            );
        }
    }
    if let Some(stored) = service.meta.get("endpoint").map(|s| s.trim()) {
        if !stored.is_empty() {
            let url = parse_https_url(stored).map_err(|_| {
                "supabase endpoint meta is not a valid https project URL".to_string()
            })?;
            let host = url.host_str().unwrap_or("").to_ascii_lowercase();
            if let Some(label) = host.strip_suffix(".supabase.co") {
                if !label.is_empty() && !label.contains('.') {
                    return Ok(host);
                }
            }
            return Err(
                "supabase endpoint meta must be the project URL (https://<ref>.supabase.co)"
                    .to_string(),
            );
        }
    }
    Err(supabase_meta_guidance())
}

fn supabase_meta_guidance() -> String {
    "supabase needs its project host: set service meta project_ref (20 lowercase alphanumerics) \
     or endpoint (https://<ref>.supabase.co)"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::super::presets::verify_endpoint;
    use super::{allowed_hosts, validate_endpoint};

    #[test]
    fn legit_provider_urls_pass() {
        assert!(validate_endpoint("openai", "https://api.openai.com/v1/models").is_ok());
        assert!(validate_endpoint("github", "https://api.github.com/user?tab=x#y").is_ok());
        assert!(validate_endpoint("openai", "https://API.OPENAI.COM/v1/models").is_ok());
        assert!(validate_endpoint("neon", "https://console.neon.tech/api/v2/projects").is_ok());
    }

    #[test]
    fn exfiltration_and_ssrf_hosts_fail() {
        // Attacker host with the victim preset: the audit's exfiltration.
        assert!(validate_endpoint("openai", "https://evil.example.com/collect").is_err());
        // Lookalike / subdomain-smuggling hosts.
        assert!(validate_endpoint("openai", "https://api.openai.com.evil.com/x").is_err());
        // Supabase wildcard is gone: anything without a service fails closed.
        assert!(validate_endpoint("supabase", "https://xyz.supabase.co/x").is_err());
        // Plain http is refused even on an allowlisted host.
        assert!(validate_endpoint("openai", "http://api.openai.com/v1/models").is_err());
        // IP literals: v4, metadata service, IPv6, loopback, explicit port.
        for hostile in [
            "https://169.254.169.254/latest/meta-data/",
            "https://192.168.1.10/admin",
            "https://10.0.0.5/",
            "https://127.0.0.1:8443/",
            "https://[::1]/",
            "https://api.openai.com:8443/v1/models",
            "https://user:pass@api.openai.com/v1/models",
            "https:///v1/models",
            "ftp://api.openai.com/x",
            // Decimal / hex / octal single-integer IP forms.
            "https://2130706433/",
            "https://0x7f000001/",
            "https://017700000001/",
            "https://[0:0:0:0:0:ffff:127.0.0.1]/",
        ] {
            assert!(
                validate_endpoint("openai", hostile).is_err(),
                "{hostile:?} must be refused"
            );
        }
    }

    #[test]
    fn parser_parity_bypass_attempts_fail() {
        // Backslash: WHATWG treats `\` as `/`, so a manual split reads host
        // `evil.example\.supabase.co` while the sender dials `evil.example`.
        assert!(validate_endpoint("openai", "https://evil.example\\.supabase.co/x").is_err());
        // Tab / newline in the authority are delimiters to the parser.
        assert!(validate_endpoint("openai", "https://api.openai.com\t.evil.com/x").is_err());
        assert!(validate_endpoint("openai", "https://api.openai.com\n.evil.com/x").is_err());
        // Encoded separators smuggled into the authority.
        assert!(validate_endpoint("openai", "https://api.openai.com%5cevil.com/x").is_err());
        assert!(validate_endpoint("openai", "https://api.openai.com%2fevil.com/x").is_err());
        // Empty userinfo halves still smuggle an `@` split.
        assert!(validate_endpoint("openai", "https://@api.openai.com/x").is_err());
        assert!(validate_endpoint("openai", "https://api.openai.com@/x").is_err());
        // Explicit default port is accepted (it IS 443); userinfo is not.
        let url = validate_endpoint("openai", "https://api.openai.com:443/v1/models")
            .expect("explicit 443 is the same origin");
        assert_eq!(url.host_str(), Some("api.openai.com"));
    }

    #[test]
    fn presets_without_http_shape_are_refused_with_guidance() {
        for preset in ["custom", "aws", "bluesky", "jules", "pix"] {
            let error = validate_endpoint(preset, "https://example.com/x").unwrap_err();
            assert!(error.contains("vault_exec"), "unexpected: {error}");
            assert!(allowed_hosts(preset).is_empty());
        }
    }

    /// Every canonical verify endpoint must survive its own preset's gate,
    /// or `vault_test_connection` breaks the moment this ships.
    #[test]
    fn verify_endpoints_survive_their_own_gate() {
        for preset in [
            "github",
            "gitlab",
            "cloudflare",
            "openrouter",
            "openai",
            "groq",
            "mistral",
            "deepseek",
            "perplexity",
            "together",
            "cohere",
            "elevenlabs",
            "anthropic",
            "gemini",
            "vercel",
            "neon",
            "stripe",
            "resend",
            "deepgram",
            "unsplash",
            "pexels",
        ] {
            let endpoint = verify_endpoint(preset)
                .unwrap_or_else(|| panic!("{preset} should have a verify endpoint"));
            assert!(
                validate_endpoint(preset, endpoint).is_ok(),
                "{preset} verify endpoint rejected: {endpoint}"
            );
        }
    }
}
