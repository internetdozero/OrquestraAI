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

/// Full catalog: AI providers, cloud/infra, dev tools, custom.
pub fn list_presets() -> Vec<PresetSpec> {
    vec![
        spec(
            "openai", "OpenAI", "authorization", "Bearer ", &[], None,
            "OPENAI_API_KEY", "GPT models and embeddings.",
            "https://platform.openai.com/api-keys", &[],
        ),
        spec(
            "anthropic", "Anthropic", "x-api-key", "",
            &[("anthropic-version", "2023-06-01")], None,
            "ANTHROPIC_API_KEY", "Claude models via x-api-key.",
            "https://console.anthropic.com/settings/keys", &[],
        ),
        spec(
            "gemini", "Google Gemini", "x-goog-api-key", "", &[], None,
            "GEMINI_API_KEY", "Gemini models via API key header.",
            "https://aistudio.google.com/apikey", &[],
        ),
        spec(
            "openrouter", "OpenRouter", "authorization", "Bearer ",
            &[("http-referer", "https://orquestra.ai")], None,
            "OPENROUTER_API_KEY", "200+ models behind one key.",
            "https://openrouter.ai/settings/keys", &[],
        ),
        spec(
            "groq", "Groq", "authorization", "Bearer ", &[], None,
            "GROQ_API_KEY", "Ultra-low-latency inference.",
            "https://console.groq.com/keys", &[],
        ),
        spec(
            "mistral", "Mistral AI", "authorization", "Bearer ", &[], None,
            "MISTRAL_API_KEY", "Mistral open and frontier models.",
            "https://console.mistral.ai/api-keys", &[],
        ),
        spec(
            "deepseek", "DeepSeek", "authorization", "Bearer ", &[], None,
            "DEEPSEEK_API_KEY", "DeepSeek chat and reasoning.",
            "https://platform.deepseek.com/api_keys", &[],
        ),
        spec(
            "perplexity", "Perplexity", "authorization", "Bearer ", &[], None,
            "PERPLEXITY_API_KEY", "Answer-engine search models.",
            "https://www.perplexity.ai/settings/api", &[],
        ),
        spec(
            "together", "Together AI", "authorization", "Bearer ", &[], None,
            "TOGETHER_API_KEY", "Open-source model cloud.",
            "https://api.together.xyz/settings/api-keys", &[],
        ),
        spec(
            "cohere", "Cohere", "authorization", "Bearer ", &[], None,
            "COHERE_API_KEY", "Command models and embeddings.",
            "https://dashboard.cohere.com/api-keys", &[],
        ),
        spec(
            "elevenlabs", "ElevenLabs", "xi-api-key", "", &[], None,
            "ELEVENLABS_API_KEY", "Voice synthesis and cloning.",
            "https://elevenlabs.io/app/settings/api-keys", &[],
        ),
        spec(
            "cloudflare", "Cloudflare", "authorization", "Bearer ", &[], None,
            "CLOUDFLARE_API_TOKEN", "Zone, DNS and Workers API.",
            "https://dash.cloudflare.com/profile/api-tokens",
            &[meta("CLOUDFLARE_ACCOUNT_ID", "Account ID", "Optional, for scoped calls")],
        ),
        spec(
            "supabase", "Supabase", "apikey", "", &[],
            Some("authorization"),
            "SUPABASE_ANON_KEY", "Postgres, auth and storage.",
            "https://supabase.com/dashboard/account/tokens",
            &[meta("SUPABASE_URL", "Project URL", "https://xyz.supabase.co")],
        ),
        spec(
            "aws", "AWS", "authorization", "Bearer ", &[], None,
            "AWS_SECRET_ACCESS_KEY", "CLI/SDK use via env vars.",
            "https://console.aws.amazon.com/iam/home#/security_credentials",
            &[
                meta("AWS_ACCESS_KEY_ID", "Access Key ID", "AKIA…"),
                meta("AWS_DEFAULT_REGION", "Default Region", "us-east-1"),
            ],
        ),
        spec(
            "vercel", "Vercel", "authorization", "Bearer ", &[], None,
            "VERCEL_API_TOKEN", "Deployments and projects API.",
            "https://vercel.com/account/tokens",
            &[],
        ),
        spec(
            "neon", "Neon", "authorization", "Bearer ", &[], None,
            "NEON_API_KEY", "Serverless Postgres management.",
            "https://console.neon.tech/app/settings/api-keys", &[],
        ),
        spec(
            "github", "GitHub", "authorization", "Bearer ",
            &[("user-agent", "OrquestraAI")], None,
            "GITHUB_TOKEN", "Repos, issues and Actions.",
            "https://github.com/settings/tokens", &[],
        ),
        spec(
            "gitlab", "GitLab", "private-token", "", &[], None,
            "GITLAB_TOKEN", "Projects and pipelines API.",
            "https://gitlab.com/-/user_settings/personal_access_tokens", &[],
        ),
        spec(
            "stripe", "Stripe", "authorization", "Bearer ", &[], None,
            "STRIPE_SECRET_KEY", "Payments and billing API.",
            "https://dashboard.stripe.com/apikeys", &[],
        ),
        spec(
            "resend", "Resend", "authorization", "Bearer ", &[], None,
            "RESEND_API_KEY", "Transactional email API.",
            "https://resend.com/api-keys", &[],
        ),
        spec(
            "tavily", "Tavily", "authorization", "Bearer ", &[], None,
            "TAVILY_API_KEY", "Search API for agents.",
            "https://app.tavily.com", &[],
        ),
        spec(
            "deepgram", "Deepgram", "authorization", "Token ", &[], None,
            "DEEPGRAM_API_KEY", "Speech-to-text and voice AI.",
            "https://console.deepgram.com", &[],
        ),
        spec(
            "jules", "Google Jules", "authorization", "Bearer ", &[], None,
            "JULES_API_KEY", "Async coding agent API.",
            "https://jules.google", &[],
        ),
        spec(
            "pix", "Pix", "authorization", "Bearer ", &[], None,
            "PIX_KEY", "Pix payment keys and tokens.",
            "", &[],
        ),
        spec(
            "unsplash", "Unsplash", "authorization", "Client-ID ", &[], None,
            "UNSPLASH_ACCESS_KEY", "Stock photos API.",
            "https://unsplash.com/developers", &[],
        ),
        spec(
            "pixabay", "Pixabay", "authorization", "", &[], None,
            "PIXABAY_API_KEY", "Images and video API.",
            "https://pixabay.com/api/docs", &[],
        ),
        spec(
            "pexels", "Pexels", "authorization", "", &[], None,
            "PEXELS_API_KEY", "Photos and video API.",
            "https://www.pexels.com/api", &[],
        ),
        spec(
            "exa", "Exa Web AI", "x-api-key", "", &[], None,
            "EXA_API_KEY", "Neural search for agents.",
            "https://dashboard.exa.ai", &[],
        ),
        spec(
            "bluesky", "Bluesky", "authorization", "Bearer ", &[], None,
            "BLUESKY_APP_PASSWORD", "AT Protocol app passwords.",
            "https://bsky.app/settings/app-passwords", &[],
        ),
        spec(
            "custom", "Custom / Generic", "authorization", "Bearer ", &[], None,
            "CUSTOM_API_KEY", "Configurable header and env var.",
            "", &[],
        ),
    ]
}

/// Look up one preset; unknown ids fall back to `custom`.
pub fn get_preset(id: &str) -> PresetSpec {
    list_presets()
        .into_iter()
        .find(|preset| preset.id == id)
        .unwrap_or_else(|| {
            spec(
                "custom", "Custom / Generic", "authorization", "Bearer ",
                &[], None, "CUSTOM_API_KEY",
                "Configurable header and env var.", "", &[],
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
