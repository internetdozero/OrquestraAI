//! SSRF-proof endpoint gate for `vault_http_request`.
//!
//! The vault key is injected into request headers, so an unrestricted URL
//! lets any IPC caller exfiltrate keys to an attacker host, or pivot into
//! the local network (RFC 1918, link-local metadata services). Every
//! endpoint must therefore be `https://` on an allowlisted host for the
//! service's preset — exact host or `*.suffix` wildcard — with IP literals,
//! userinfo and non-443 ports rejected outright.
//!
//! Two residual notes, by design: (1) DNS resolution is left to the TLS
//! stack — a rebinding that points an allowlisted name at link-local fails
//! certificate verification in `reqwest`/`rustls`, which we do not disable;
//! (2) redirects are fenced to same-host in `http::execute_once`, so a
//! compromised API cannot bounce the call (and its headers) elsewhere.

/// Hosts a preset may call. `*.` entries match one-or-more left labels
/// (`xyz.supabase.co` matches `*.supabase.co`); anything else is exact.
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
        "supabase" => &["*.supabase.co"],
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
        _ => &[],
    }
}

/// Reject anything that is not an `https://` URL on the preset's allowlist.
pub fn validate_endpoint(preset_id: &str, endpoint: &str) -> Result<(), String> {
    let rest = endpoint
        .strip_prefix("https://")
        .ok_or_else(|| "vault endpoint must use https:// (plain http is refused)".to_string())?;
    // Authority is up to the first path/query/fragment delimiter.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err("vault endpoint has no host".to_string());
    }
    // `user:pass@host` would smuggle credentials or confuse host parsing.
    if authority.contains('@') {
        return Err("vault endpoint must not contain userinfo".to_string());
    }
    // Split host from port, tolerating bracketed IPv6 (rejected below).
    let host = authority
        .strip_prefix('[')
        .and_then(|inner| inner.split(']').next())
        .unwrap_or_else(|| authority.split(':').next().unwrap_or(""));
    if host.is_empty() {
        return Err("vault endpoint has no host".to_string());
    }
    if is_ip_literal(host) {
        return Err(format!(
            "vault endpoint host {host:?} is an IP literal: only provider hostnames are allowed"
        ));
    }
    // Only the TLS default: an explicit port is a tunneling smell.
    let after_host = authority.strip_prefix(host).unwrap_or("");
    // Bracketed form consumed the brackets above; re-derive the port tail.
    let port_tail = if authority.starts_with('[') {
        authority.split(']').nth(1).unwrap_or("")
    } else {
        after_host
    };
    if let Some(port) = port_tail.strip_prefix(':') {
        if port != "443" {
            return Err(format!(
                "vault endpoint port {port:?} is not allowed (443 only)"
            ));
        }
    }
    let allowed = allowed_hosts(preset_id);
    if allowed.is_empty() {
        return Err(format!(
            "the {preset_id} preset has no allowlisted HTTP hosts: use vault_exec for this service"
        ));
    }
    let host_lc = host.to_ascii_lowercase();
    let permitted = allowed.iter().any(|entry| {
        if let Some(suffix) = entry.strip_prefix("*.") {
            host_lc.len() > suffix.len()
                && host_lc.ends_with(suffix)
                && host_lc.as_bytes()[host_lc.len() - suffix.len() - 1] == b'.'
        } else {
            host_lc == *entry
        }
    });
    if !permitted {
        return Err(format!(
            "vault endpoint host {host:?} is not allowlisted for the {preset_id} preset"
        ));
    }
    Ok(())
}

/// Canonical dotted-quad, colon-bearing IPv6, and bracket-stripped forms.
/// Decimal/octal/hex single-integer forms (`2130706433`) are not parsed as
/// IP here, but they still die at the allowlist check: no preset lists them.
fn is_ip_literal(host: &str) -> bool {
    if host.contains(':') {
        return true;
    }
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 3 && part.bytes().all(|b| b.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::super::presets::verify_endpoint;
    use super::{allowed_hosts, validate_endpoint};

    #[test]
    fn legit_provider_urls_pass() {
        assert!(validate_endpoint("openai", "https://api.openai.com/v1/models").is_ok());
        assert!(validate_endpoint("github", "https://api.github.com/user?tab=x#y").is_ok());
        assert!(validate_endpoint("supabase", "https://xyz.supabase.co/rest/v1/t").is_ok());
        assert!(validate_endpoint("openai", "https://API.OPENAI.COM/v1/models").is_ok());
        assert!(validate_endpoint("neon", "https://console.neon.tech/api/v2/projects").is_ok());
    }

    #[test]
    fn exfiltration_and_ssrf_hosts_fail() {
        // Attacker host with the victim preset: the audit's exfiltration.
        assert!(validate_endpoint("openai", "https://evil.example.com/collect").is_err());
        // Lookalike / subdomain-smuggling hosts.
        assert!(validate_endpoint("openai", "https://api.openai.com.evil.com/x").is_err());
        assert!(validate_endpoint("supabase", "https://supabase.co.evil.com/x").is_err());
        // Wildcard must not match the bare suffix.
        assert!(validate_endpoint("supabase", "https://supabase.co/x").is_err());
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
        ] {
            assert!(
                validate_endpoint("openai", hostile).is_err(),
                "{hostile:?} must be refused"
            );
        }
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
