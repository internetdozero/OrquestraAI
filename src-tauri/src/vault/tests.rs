//! Unit tests: secret redaction, key rotation, preset headers.

use super::http::{effective_headers, scrub_body, strip_response_headers};
use super::presets::{build_exec_env, build_headers, get_preset};
use super::store::{first_key_index_in, health_for_status, mask_key, next_key_index, next_key_index_in, normalize_tag, KeyHealth, VaultKey, VaultService};
use std::collections::HashMap;

fn test_service(preset: &str, secrets: &[&str]) -> VaultService {
    VaultService {
        id: "svc".to_string(),
        label: "Svc".to_string(),
        preset: preset.to_string(),
        description: String::new(),
        keys: secrets
            .iter()
            .enumerate()
            .map(|(index, secret)| VaultKey {
                id: format!("key-{index}"),
                secret: secret.to_string(),
                tag: None,
                health: KeyHealth::Healthy,
                cooldown_until_ms: 0,
                failures: 0,
            }).collect(),
        active: 0,
        custom_header: None,
        custom_prefix: None,
        custom_env: None,
        meta: HashMap::new(),
        updated_at_ms: 0,
    }
}

#[test]
fn preset_headers_match_canonical_specs() {
    let github = build_headers("github", "ghp_secret123", None, None, None);
    assert!(github.iter().any(|(name, value)| name == "authorization" && value == "Bearer ghp_secret123"));
    assert!(github.iter().any(|(name, value)| name.eq_ignore_ascii_case("user-agent") && value == "OrquestraAI"));

    let anthropic = build_headers("anthropic", "sk-ant-abc", None, None, None);
    assert!(anthropic.iter().any(|(name, value)| name == "x-api-key" && value == "sk-ant-abc"));
    assert!(anthropic.iter().any(|(name, value)| name == "anthropic-version" && value == "2023-06-01"));

    let gemini = build_headers("gemini", "AIza-key", None, None, None);
    assert!(gemini.iter().any(|(name, value)| name == "x-goog-api-key" && value == "AIza-key"));

    let cloudflare = build_headers("cloudflare", "cf_token", None, None, None);
    assert!(cloudflare.iter().any(|(name, value)| name == "authorization" && value == "Bearer cf_token"));

    let custom = build_headers("custom", "raw", Some("X-Custom-Auth"), Some("Token "), None);
    assert!(custom.iter().any(|(name, value)| name == "X-Custom-Auth" && value == "Token raw"));

    assert_eq!(get_preset("nope").id, "custom");
    let env = build_exec_env("github", "ghp_secret123", None);
    assert!(env.iter().any(|(name, value)| name == "GITHUB_TOKEN" && value == "ghp_secret123"));
}

#[test]
fn key_rotation_skips_cooldown_and_invalid_keys() {
    let mut keys = vec![
        VaultKey { id: "key-0".to_string(), secret: "a".to_string(), tag: None, health: KeyHealth::Healthy, cooldown_until_ms: 0, failures: 0 },
        VaultKey { id: "key-1".to_string(), secret: "b".to_string(), tag: None, health: KeyHealth::RateLimited, cooldown_until_ms: 9_999_999_999, failures: 1 },
        VaultKey { id: "key-2".to_string(), secret: "c".to_string(), tag: None, health: KeyHealth::Healthy, cooldown_until_ms: 0, failures: 0 },
    ];
    assert_eq!(next_key_index(&keys, 0, 1000), Some(2));
    keys[2].health = KeyHealth::InvalidKey;
    assert_eq!(next_key_index(&keys, 0, 1000), None);

    assert_eq!(health_for_status(429), Some(KeyHealth::RateLimited));
    assert_eq!(health_for_status(402), Some(KeyHealth::QuotaExceeded));
    assert_eq!(health_for_status(403), Some(KeyHealth::QuotaExceeded));
    assert_eq!(health_for_status(401), Some(KeyHealth::InvalidKey));
    assert_eq!(health_for_status(200), None);
}

#[test]
fn secret_redaction_strips_headers_body_and_exec_output() {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer secret".to_string());
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers.insert("Set-Cookie".to_string(), "session=abc".to_string());
    let stripped = strip_response_headers(&headers);
    assert!(!stripped.contains_key("Authorization"));
    assert!(!stripped.contains_key("Set-Cookie"));
    assert_eq!(stripped["Content-Type"], "application/json");

    let body = scrub_body("token sk-live-1234 ok sk-live-1234", &["sk-live-1234"]);
    assert!(!body.contains("sk-live-1234"));
    assert!(body.contains("[REDACTED]"));

    let service = test_service("openai", &["sk-secret-9999"]);
    let secret = "sk-secret-9999".to_string();
    let outgoing = effective_headers(&service, &secret, &HashMap::from([("Authorization".to_string(), "Bearer attacker".to_string())]));
    assert!(!outgoing.iter().any(|(_, value)| value == "Bearer attacker"));
    assert!(outgoing.iter().any(|(_, value)| value == "Bearer sk-secret-9999"));

    let masked = mask_key("sk-ant-abcdefgh9f82");
    assert!(!masked.contains("abcdefgh"));
    assert!(masked.ends_with("9f82"));
    assert_eq!(mask_key("short"), "********");
}

#[test]
fn exec_env_uses_preset_var_and_custom_override() {
    let service = test_service("anthropic", &["sk-ant-x"]);
    let env = super::exec::child_env(&service, "sk-ant-x");
    assert_eq!(env["ANTHROPIC_API_KEY"], "sk-ant-x");
    let scrubbed = super::exec::scrub_output("echo sk-ant-x done", &["sk-ant-x".to_string()]);
    assert!(!scrubbed.contains("sk-ant-x"));
}

#[test]
fn exec_command_accepts_string_and_string_array() {
    let service = test_service("openai", &["sk-secret-9999"]);
    let secret = "sk-secret-9999";

    let string_form = serde_json::Value::String("echo hello".to_string());
    let (code, stdout, _) = super::exec::run_command(&service, secret, &string_form)
        .expect("string command should run");
    assert_eq!(code, 0);
    assert!(stdout.contains("hello"));

    let array_form = serde_json::json!(["echo", "hello-array"]);
    let (code, stdout, _) = super::exec::run_command(&service, secret, &array_form)
        .expect("array command should run");
    assert_eq!(code, 0);
    assert!(stdout.contains("hello-array"));

    // A secret echoed by the child must never come back verbatim.
    let leak = serde_json::Value::String("echo $OPENAI_API_KEY".to_string());
    let (_, stdout, _) = super::exec::run_command(&service, secret, &leak)
        .expect("env echo should run");
    assert!(!stdout.contains(secret));

    assert!(super::exec::run_command(&service, secret, &serde_json::json!([])).is_err());
    assert!(super::exec::run_command(&service, secret, &serde_json::json!(["echo", 7])).is_err());
    assert!(super::exec::run_command(&service, secret, &serde_json::json!(7)).is_err());
}

#[test]
fn verify_endpoints_cover_every_non_custom_preset() {
    use super::presets::verify_endpoint;
    for preset in super::presets::list_presets() {
        // No universal ping: unknown shape, SigV4 signing, or per-project /
        // POST-only APIs. Those keys stay CLI/env-usable with manual health.
        if matches!(preset.id.as_str(), "custom" | "aws" | "supabase" | "tavily" | "jules" | "pix" | "pixabay" | "exa" | "bluesky") {
            assert_eq!(verify_endpoint(&preset.id), None);
        } else {
            let endpoint = verify_endpoint(&preset.id)
                .unwrap_or_else(|| panic!("{} needs a verify endpoint", preset.id));
            assert!(endpoint.starts_with("https://"), "{endpoint} must be https");
        }
    }
    assert_eq!(verify_endpoint("unknown-preset"), None);
}

#[test]
fn test_connection_outcome_updates_key_health() {
    use super::store::{health_for_status, record_result};
    // 429 cools the key down and fails over to the reserve.
    let mut service = test_service("github", &["ghp_primary", "ghp_backup"]);
    record_result(&mut service, 0, 429);
    assert_eq!(service.keys[0].health, KeyHealth::RateLimited);
    assert!(service.keys[0].cooldown_until_ms > 0);
    assert_eq!(service.active, 1);
    assert_eq!(health_for_status(429), Some(KeyHealth::RateLimited));
    // A later 200 heals the key that served it.
    record_result(&mut service, 1, 200);
    assert_eq!(service.keys[1].health, KeyHealth::Healthy);
    // 401 marks the key invalid without a cooldown timestamp.
    let mut single = test_service("openai", &["sk-bad"]);
    record_result(&mut single, 0, 401);
    assert_eq!(single.keys[0].health, KeyHealth::InvalidKey);
    assert_eq!(single.keys[0].cooldown_until_ms, 0);
}

#[test]
fn preset_links_mirrors_and_meta_fields() {
    use super::presets::{build_headers, get_preset};
    assert_eq!(super::presets::list_presets().len(), 30);
    let gh = get_preset("github");
    assert!(gh.key_url.contains("github.com/settings/tokens"));
    assert!(gh.meta_fields.is_empty());
    // Supabase sends the key twice: apikey + mirrored Bearer authorization.
    let sub = build_headers("supabase", "sb-secret", None, None, None);
    assert!(sub.iter().any(|(n, v)| n == "apikey" && v == "sb-secret"));
    assert!(sub.iter().any(|(n, v)| n == "authorization" && v == "Bearer sb-secret"));
    // GitLab uses its private-token header with no prefix.
    let gl = build_headers("gitlab", "glpat-x", None, None, None);
    assert!(gl.iter().any(|(n, v)| n == "private-token" && v == "glpat-x"));
    // AWS declares companion env fields for the modal.
    let aws = get_preset("aws");
    assert!(aws.meta_fields.iter().any(|f| f.key == "AWS_ACCESS_KEY_ID"));
    assert!(aws.meta_fields.iter().any(|f| f.key == "AWS_DEFAULT_REGION"));
    assert_eq!(super::presets::verify_endpoint("aws"), None);
}

#[test]
fn per_key_tags_scope_failover() {
    let tagged = |secret: &str, tag: Option<&str>| VaultKey {
        id: secret.to_string(),
        secret: secret.to_string(),
        tag: tag.map(str::to_string),
        health: KeyHealth::Healthy,
        cooldown_until_ms: 0,
        failures: 0,
    };
    let keys = vec![
        tagged("workers-key", Some("workers")),
        tagged("d1-a", Some("d1")),
        tagged("d1-b", Some("d1")),
    ];
    // Tag matching is case-insensitive and trims whitespace.
    assert_eq!(normalize_tag("  Workers "), Some("workers".to_string()));
    assert_eq!(normalize_tag("   "), None);
    // First pick stays inside the tag scope.
    assert_eq!(first_key_index_in(&keys, 0, 0, Some("d1")), Some(1));
    assert_eq!(first_key_index_in(&keys, 0, 0, Some("D1")), Some(1));
    assert_eq!(first_key_index_in(&keys, 0, 0, Some("workers")), Some(0));
    assert_eq!(first_key_index_in(&keys, 0, 0, Some("missing")), None);
    // Failover never leaves the tag.
    assert_eq!(next_key_index_in(&keys, 1, 0, Some("d1")), Some(2));
    assert_eq!(next_key_index_in(&keys, 2, 0, Some("d1")), Some(1));
    assert_eq!(next_key_index_in(&keys, 0, 0, Some("workers")), None);
    // Untagged scope keeps the legacy whole-pool behavior.
    assert_eq!(next_key_index(&keys, 0, 0), Some(1));
    // Serialized old vaults without tags still load.
    let legacy: Vec<VaultKey> = serde_json::from_str(r#"[{"id":"key-0","secret":"abc"}]"#).expect("legacy key");
    assert_eq!(legacy[0].tag, None);
}

#[test]
fn new_preset_auth_schemas_match_canonical_specs() {
    let deepgram = build_headers("deepgram", "dg-key", None, None, None);
    assert!(deepgram.iter().any(|(n, v)| n == "authorization" && v == "Token dg-key"));
    assert_eq!(get_preset("deepgram").env_var, "DEEPGRAM_API_KEY");
    let unsplash = build_headers("unsplash", "pub-key", None, None, None);
    assert!(unsplash.iter().any(|(n, v)| n == "authorization" && v == "Client-ID pub-key"));
    assert_eq!(get_preset("unsplash").env_var, "UNSPLASH_ACCESS_KEY");
    let exa = build_headers("exa", "ex-key", None, None, None);
    assert!(exa.iter().any(|(n, v)| n == "x-api-key" && v == "ex-key"));
    assert_eq!(get_preset("exa").env_var, "EXA_API_KEY");
    let stripe = build_headers("stripe", "sk_live", None, None, None);
    assert!(stripe.iter().any(|(n, v)| n == "authorization" && v == "Bearer sk_live"));
    assert_eq!(get_preset("stripe").env_var, "STRIPE_SECRET_KEY");
    assert_eq!(get_preset("jules").env_var, "JULES_API_KEY");
    assert_eq!(get_preset("pix").env_var, "PIX_KEY");
    assert_eq!(get_preset("pixabay").env_var, "PIXABAY_API_KEY");
    assert_eq!(get_preset("pexels").env_var, "PEXELS_API_KEY");
    assert_eq!(get_preset("bluesky").env_var, "BLUESKY_APP_PASSWORD");
}

#[test]
fn saved_tag_resolution_distinguishes_clear_from_keep() {
    use super::store::resolve_saved_tag;
    // Explicit entries win, blanks clear.
    let workers = Some(Some(" Workers ".to_string()));
    assert_eq!(
        resolve_saved_tag(workers.as_ref(), Some("old".to_string())),
        Some("workers".to_string())
    );
    let cleared: Option<Option<String>> = Some(None);
    assert_eq!(resolve_saved_tag(cleared.as_ref(), Some("old".to_string())), None);
    let blank = Some(Some("  ".to_string()));
    assert_eq!(resolve_saved_tag(blank.as_ref(), Some("old".to_string())), None);
    // No entry (no array / short array) keeps the previous tag.
    assert_eq!(resolve_saved_tag(None, Some("old".to_string())), Some("old".to_string()));
    assert_eq!(resolve_saved_tag(None, None), None);
}

#[test]
fn service_meta_round_trips_into_child_env() {
    use std::collections::HashMap;
    let mut service = test_service("aws", &["aws-secret"]);
    service.meta = HashMap::from([("AWS_DEFAULT_REGION".to_string(), "us-east-1".to_string())]);
    let env = super::exec::child_env(&service, "aws-secret");
    assert_eq!(env["AWS_SECRET_ACCESS_KEY"], "aws-secret");
    assert_eq!(env["AWS_DEFAULT_REGION"], "us-east-1");
}
