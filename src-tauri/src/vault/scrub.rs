//! Encoding-aware secret redaction for vault output paths.
//!
//! Literal substitution alone is trivially bypassed: a child process (or a
//! chatty provider CLI) can emit the key base64/hex/URL-encoded or reversed
//! and sail past a raw-string filter. Every secret is therefore expanded to
//! its common derived forms before masking, so all of them redact to
//! `[REDACTED]` in exec stdout/stderr and HTTP bodies alike.
//!
//! Deliberately dependency-free: the three encoders below are tiny, auditable
//! loops with no edge cases beyond ASCII secrets (vault keys always are).

/// Marker replacing any secret or derived form.
const MARKER: &str = "[REDACTED]";

/// Raw secrets shorter than this are ignored (unchanged historical rule).
const MIN_RAW_LEN: usize = 4;

/// Derived forms are only generated for secrets at least this long: the
/// base64 of a 4-char secret (or its reversal) is short enough to collide
/// with innocent output and would spray false `[REDACTED]` markers.
const MIN_DERIVED_LEN: usize = 8;

/// All strings that must never reach the agent for one secret: the raw key
/// plus its base64 (standard + URL-safe, padded + unpadded), hex
/// (lower + upper), URL-encoded and reversed forms.
pub fn secret_variants(secret: &str) -> Vec<String> {
    let mut out = Vec::new();
    if secret.len() < MIN_RAW_LEN {
        return out;
    }
    push_unique(&mut out, secret.to_string());
    if secret.len() < MIN_DERIVED_LEN {
        return out;
    }
    let b64 = base64_encode(secret.as_bytes(), false);
    let b64url = base64_encode(secret.as_bytes(), true);
    push_unique(&mut out, b64.clone());
    push_unique(&mut out, b64url.clone());
    push_unique(&mut out, b64.trim_end_matches('=').to_string());
    push_unique(&mut out, b64url.trim_end_matches('=').to_string());
    push_unique(&mut out, hex_encode(secret.as_bytes(), false));
    push_unique(&mut out, hex_encode(secret.as_bytes(), true));
    push_unique(&mut out, url_encode(secret.as_bytes()));
    push_unique(&mut out, secret.chars().rev().collect());
    out
}

fn push_unique(out: &mut Vec<String>, candidate: String) {
    if candidate.len() >= MIN_RAW_LEN && !out.contains(&candidate) {
        out.push(candidate);
    }
}

/// Replace every variant of every secret in `text` with `[REDACTED]`.
pub fn scrub_with_variants(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for secret in secrets {
        for variant in secret_variants(secret) {
            out = out.replace(&variant, MARKER);
        }
    }
    out
}

fn base64_encode(bytes: &[u8], url_safe: bool) -> String {
    const STD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let table = if url_safe { URL } else { STD };
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut n: u32 = 0;
        for (i, byte) in chunk.iter().enumerate() {
            n |= (*byte as u32) << (16 - 8 * i);
        }
        let pad = 3 - chunk.len();
        for i in 0..4 - pad {
            out.push(table[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
        for _ in 0..pad {
            out.push('=');
        }
    }
    out
}

fn hex_encode(bytes: &[u8], upper: bool) -> String {
    const LO: &[u8; 16] = b"0123456789abcdef";
    const HI: &[u8; 16] = b"0123456789ABCDEF";
    let table = if upper { HI } else { LO };
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(table[(byte >> 4) as usize] as char);
        out.push(table[(byte & 15) as usize] as char);
    }
    out
}

/// Percent-encode every byte outside the RFC 3986 unreserved set.
fn url_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::new();
    for byte in bytes {
        let unreserved = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(*byte as char);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 15) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{scrub_with_variants, secret_variants};

    const SECRET: &str = "sk-ant-secret-9999";

    #[test]
    fn encoders_match_known_vectors() {
        assert_eq!(super::base64_encode(b"Man", false), "TWFu");
        assert_eq!(super::base64_encode(b"Ma", false), "TWE=");
        assert_eq!(super::base64_encode(b"f>>?", false), "Zj4+Pw==");
        assert_eq!(super::base64_encode(b"f>>?", true), "Zj4-Pw==");
        assert_eq!(super::hex_encode(b"Az", false), "417a");
        assert_eq!(super::hex_encode(b"Az", true), "417A");
        assert_eq!(super::url_encode(b"a b+c"), "a%20b%2Bc");
    }

    #[test]
    fn variants_cover_the_bypass_alphabet() {
        let variants = secret_variants(SECRET);
        assert!(variants.contains(&SECRET.to_string()));
        assert!(variants.iter().any(|v| v.contains('=') || v.len() > SECRET.len()));
        assert!(variants.contains(&SECRET.chars().rev().collect::<String>()));
        // Short secrets redact raw-only: no derived forms to false-positive on.
        assert_eq!(secret_variants("abc"), Vec::<String>::new());
        assert_eq!(secret_variants("abcd"), vec!["abcd".to_string()]);
    }

    /// The exact attack from the audit: `echo $SECRET | base64` output must
    /// not survive scrubbing, in any common encoding.
    #[test]
    fn encoded_exfiltration_is_masked() {
        let b64 = super::base64_encode(SECRET.as_bytes(), false);
        let b64url = super::base64_encode(SECRET.as_bytes(), true);
        let hex = super::hex_encode(SECRET.as_bytes(), false);
        let rev: String = SECRET.chars().rev().collect();
        for leaked in [
            SECRET.to_string(),
            b64.clone(),
            b64.trim_end_matches('=').to_string(),
            b64url,
            hex.to_uppercase(),
            super::url_encode(SECRET.as_bytes()),
            rev,
        ] {
            let out = scrub_with_variants(&format!("result: {leaked}"), &[SECRET]);
            assert!(!out.contains(&leaked), "leaked form survived: {leaked}");
            assert!(out.contains("[REDACTED]"), "no marker for: {leaked}");
        }
    }

    #[test]
    fn innocent_text_passes_through_untouched() {
        let out = scrub_with_variants("everything is fine, exit 0", &[SECRET]);
        assert_eq!(out, "everything is fine, exit 0");
    }

    /// End-to-end through the real spawn path: `printf` emits the base64
    /// form — exactly what `echo $SECRET | base64` would return — and the
    /// IPC-bound stdout must come back masked, not verbatim.
    #[test]
    fn encoded_child_output_is_scrubbed_end_to_end() {
        use super::super::exec;
        use super::super::store::{KeyHealth, VaultKey, VaultService};
        let service = VaultService {
            id: "svc".to_string(),
            label: "Svc".to_string(),
            preset: "openai".to_string(),
            description: String::new(),
            keys: vec![VaultKey {
                id: "key-0".to_string(),
                secret: SECRET.to_string(),
                tag: None,
                health: KeyHealth::Healthy,
                cooldown_until_ms: 0,
                failures: 0,
            }],
            active: 0,
            custom_header: None,
            custom_prefix: None,
            custom_env: None,
            meta: std::collections::HashMap::new(),
            updated_at_ms: 0,
        };
        let b64 = super::base64_encode(SECRET.as_bytes(), false);
        let payload = serde_json::json!(["printf", "%s", b64]);
        let (code, stdout, _) =
            exec::run_command(&service, SECRET, &payload).expect("printf must run");
        assert_eq!(code, 0);
        assert!(!stdout.contains(&b64), "base64 exfiltration survived: {stdout}");
        assert!(stdout.contains("[REDACTED]"));
    }
}
