//! Canonical auth patterns for popular developer services.
//!
//! Each preset knows how to inject a blind key into an outgoing HTTP
//! request (header name + value prefix + extra static headers, plus an
//! optional mirrored `Authorization` header for providers like Supabase
//! that want the key twice) and which env var a child process expects.
//! `custom` covers everything else via a caller-supplied header template
//! or env name. Non-secret extras (account ids, regions, project URLs)
//! travel in the service `meta` map, never in the key pool.

use serde::{Deserialize, Serialize};

mod catalog;

pub use catalog::list_presets;

/// One non-secret companion field (account id, region, project URL).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaField {
    /// Env var name, e.g. `AWS_DEFAULT_REGION`.
    pub key: String,
    pub label: String,
    pub placeholder: String,
}

/// One preset in the catalog surfaced to the UI dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetSpec {
    pub id: String,
    pub label: String,
    /// Header that carries the credential (e.g. `authorization`).
    pub header: String,
    /// Prefix before the raw key inside the header value (`Bearer `).
    pub prefix: String,
    /// Static extra headers always sent with this preset.
    pub extra_headers: Vec<(String, String)>,
    /// Second header that also carries `Bearer <key>` (Supabase-style).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_mirror: Option<String>,
    /// Env var a subprocess expects for this service.
    pub env_var: String,
    pub description: String,
    /// Where the operator generates a key (`""` = none).
    #[serde(default)]
    pub key_url: String,
    /// Non-secret companion fields collected by the UI.
    #[serde(default)]
    pub meta_fields: Vec<MetaField>,
}

fn meta(key: &str, label: &str, placeholder: &str) -> MetaField {
    MetaField {
        key: key.to_string(),
        label: label.to_string(),
        placeholder: placeholder.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn spec(
    id: &str,
    label: &str,
    header: &str,
    prefix: &str,
    extra: &[(&str, &str)],
    mirror: Option<&str>,
    env_var: &str,
    desc: &str,
    key_url: &str,
    meta_fields: &[MetaField],
) -> PresetSpec {
    PresetSpec {
        id: id.to_string(),
        label: label.to_string(),
        header: header.to_string(),
        prefix: prefix.to_string(),
        extra_headers: extra
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        auth_mirror: mirror.map(str::to_string),
        env_var: env_var.to_string(),
        description: desc.to_string(),
        key_url: key_url.to_string(),
        meta_fields: meta_fields.to_vec(),
    }
}

/// Look up one preset; unknown ids fall back to `custom`.
pub fn get_preset(id: &str) -> PresetSpec {
    list_presets()
        .into_iter()
        .find(|preset| preset.id == id)
        .unwrap_or_else(|| {
            spec(
                "custom",
                "Custom / Generic",
                "authorization",
                "Bearer ",
                &[],
                None,
                "CUSTOM_API_KEY",
                "Configurable header and env var.",
                "",
                &[],
            )
        })
}

/// Build the auth + extra headers for one blind key. The raw key never
/// leaves this function except inside the header value sent upstream.
pub fn build_headers(
    preset_id: &str,
    secret: &str,
    custom_header: Option<&str>,
    custom_prefix: Option<&str>,
    custom_env: Option<&str>,
) -> Vec<(String, String)> {
    let preset = get_preset(preset_id);
    let header = custom_header
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or(preset.header);
    let prefix = custom_prefix.map(str::to_string).unwrap_or(preset.prefix);
    let _ = custom_env;
    let mut out = vec![(header, format!("{prefix}{secret}"))];
    if let Some(mirror) = preset.auth_mirror {
        out.push((mirror, format!("Bearer {secret}")));
    }
    out.extend(preset.extra_headers.clone());
    out
}

/// Env pairs injected into a child process for `vault_exec`.
pub fn build_exec_env(
    preset_id: &str,
    secret: &str,
    custom_env: Option<&str>,
) -> Vec<(String, String)> {
    let preset = get_preset(preset_id);
    let name = custom_env
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or(preset.env_var);
    vec![(name, secret.to_string())]
}

/// Canonical lightweight endpoint proving a key is valid. `None` means the
/// preset has no safe universal ping: `custom` (unknown shape), `aws`
/// (SigV4 signing, no bearer check), `supabase`/`tavily` (per-project URLs
/// or POST-only APIs), `jules`/`pix`/`pixabay`/`exa`/`bluesky` (per-account
/// URLs, query-key auth, or session flows). Those keys stay CLI/env-usable
/// with manual health.
pub fn verify_endpoint(preset_id: &str) -> Option<&'static str> {
    match preset_id {
        "github" => Some("https://api.github.com/user"),
        "gitlab" => Some("https://gitlab.com/api/v4/user"),
        "cloudflare" => Some("https://api.cloudflare.com/client/v4/user/tokens/verify"),
        "openrouter" => Some("https://openrouter.ai/api/v1/auth/key"),
        "openai" => Some("https://api.openai.com/v1/models"),
        "groq" => Some("https://api.groq.com/openai/v1/models"),
        "mistral" => Some("https://api.mistral.ai/v1/models"),
        "deepseek" => Some("https://api.deepseek.com/v1/models"),
        "perplexity" => Some("https://api.perplexity.ai/models"),
        "together" => Some("https://api.together.xyz/v1/models"),
        "cohere" => Some("https://api.cohere.com/v1/models"),
        "elevenlabs" => Some("https://api.elevenlabs.com/v1/models"),
        "anthropic" => Some("https://api.anthropic.com/v1/models"),
        "gemini" => Some("https://generativelanguage.googleapis.com/v1beta/models"),
        "vercel" => Some("https://api.vercel.com/v2/user"),
        "neon" => Some("https://console.neon.tech/api/v2/projects"),
        "stripe" => Some("https://api.stripe.com/v1/balance"),
        "resend" => Some("https://api.resend.com/api-keys"),
        "deepgram" => Some("https://api.deepgram.com/v1/projects"),
        "unsplash" => Some("https://api.unsplash.com/me"),
        "pexels" => Some("https://api.pexels.com/v1/curated?per_page=1"),
        _ => None,
    }
}
