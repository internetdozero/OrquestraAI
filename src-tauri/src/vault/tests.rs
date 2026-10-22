//! Unit tests: secret redaction, key rotation, preset headers.

use super::http::{effective_headers, scrub_body, strip_response_headers};
use super::presets::{build_exec_env, build_headers, get_preset};
use super::store::{
    first_key_index_in, health_for_status, mask_key, next_key_index, next_key_index_in,
    normalize_tag, KeyHealth, VaultKey, VaultService,
};
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
            })
            .collect(),
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
    assert!(github
        .iter()
        .any(|(name, value)| name == "authorization" && value == "Bearer ghp_secret123"));
    assert!(github
        .iter()
        .any(|(name, value)| name.eq_ignore_ascii_case("user-agent") && value == "OrquestraAI"));

    let anthropic = build_headers("anthropic", "sk-ant-abc", None, None, None);
    assert!(anthropic
        .iter()
        .any(|(name, value)| name == "x-api-key" && value == "sk-ant-abc"));
    assert!(anthropic
        .iter()
        .any(|(name, value)| name == "anthropic-version" && value == "2023-06-01"));

    let gemini = build_headers("gemini", "AIza-key", None, None, None);
    assert!(gemini
        .iter()
        .any(|(name, value)| name == "x-goog-api-key" && value == "AIza-key"));

    let cloudflare = build_headers("cloudflare", "cf_token", None, None, None);
    assert!(cloudflare
        .iter()
        .any(|(name, value)| name == "authorization" && value == "Bearer cf_token"));

    let custom = build_headers("custom", "raw", Some("X-Custom-Auth"), Some("Token "), None);
    assert!(custom
        .iter()
        .any(|(name, value)| name == "X-Custom-Auth" && value == "Token raw"));

    assert_eq!(get_preset("nope").id, "custom");
    let env = build_exec_env("github", "ghp_secret123", None);
    assert!(env
        .iter()
        .any(|(name, value)| name == "GITHUB_TOKEN" && value == "ghp_secret123"));
}

#[test]
fn key_rotation_skips_cooldown_and_invalid_keys() {
    let mut keys = vec![
        VaultKey {
            id: "key-0".to_string(),
            secret: "a".to_string(),
            tag: None,
            health: KeyHealth::Healthy,
            cooldown_until_ms: 0,
            failures: 0,
        },
        VaultKey {
            id: "key-1".to_string(),
            secret: "b".to_string(),
            tag: None,
            health: KeyHealth::RateLimited,
            cooldown_until_ms: 9_999_999_999,
            failures: 1,
        },
        VaultKey {
            id: "key-2".to_string(),
            secret: "c".to_string(),
            tag: None,
            health: KeyHealth::Healthy,
            cooldown_until_ms: 0,
            failures: 0,
        },
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
    let outgoing = effective_headers(
        &service,
        &secret,
        &HashMap::from([("Authorization".to_string(), "Bearer attacker".to_string())]),
    );
    assert!(!outgoing.iter().any(|(_, value)| value == "Bearer attacker"));
    assert!(outgoing
        .iter()
        .any(|(_, value)| value == "Bearer sk-secret-9999"));

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
fn exec_rejects_shell_strings_and_non_allowlisted_programs() {
    let service = test_service("openai", &["sk-secret-9999"]);
    let secret = "sk-secret-9999";

    // Shell strings are rejected outright — no `sh -c` path exists anymore,
    // so pipes, redirections and expansions never reach an interpreter.
    for hostile in [
        "echo hello",
        "echo hello; rm -rf /",
        "echo $(touch /tmp/pwned)",
        "echo `touch /tmp/pwned`",
        "echo hello | tee /tmp/pwned",
        "echo hi > /tmp/pwned",
        "echo $OPENAI_API_KEY",
    ] {
        let payload = serde_json::Value::String(hostile.to_string());
        assert!(
            super::exec::run_command(&service, secret, &payload).is_err(),
            "{hostile:?} must be refused"
        );
    }

    // Array form: only bare allowlisted names. Shells, script runtimes,
    // downloaders and any path (absolute, relative, traversal) are refused
    // before the OS loader is ever consulted.
    for argv in [
        vec!["sh", "-c", "echo hi"],
        vec!["bash", "-c", "echo hi"],
        vec!["python3", "-c", "print(1)"],
        vec!["curl", "https://example.com"],
        vec!["./echo", "hi"],
        vec!["/bin/echo", "hi"],
        vec!["/usr/bin/printf", "%s", "hi"],
        vec!["echo; rm -rf /", "hi"],
        vec!["echo\0hi", "hi"],
        vec!["echo", ""],
        vec!["echo", "hi\0"],
    ] {
        let payload = serde_json::Value::Array(
            argv.into_iter()
                .map(|part| serde_json::Value::String(part.to_string()))
                .collect(),
        );
        assert!(super::exec::run_command(&service, secret, &payload).is_err());
    }

    // Oversized input fails validation without spawning anything.
    let huge_argv = vec!["echo".to_string()]
        .into_iter()
        .chain(vec!["x".to_string(); 200])
        .collect::<Vec<_>>();
    assert!(super::exec::run_argv(&service, secret, &huge_argv).is_err());
    let huge_arg = vec!["echo".to_string(), "x".repeat(1_000_001)];
    assert!(super::exec::run_argv(&service, secret, &huge_arg).is_err());

    // Legit array form still runs, and `$VAR` stays literal with no shell.
    let array_form = serde_json::json!(["echo", "hello-array"]);
    let (code, stdout, _) = super::exec::run_command(&service, secret, &array_form)
        .expect("allowlisted array command should run");
    assert_eq!(code, 0);
    assert!(stdout.contains("hello-array"));

    // A secret echoed by the child must never come back verbatim.
    let literal = serde_json::json!(["echo", "$OPENAI_API_KEY"]);
    let (_, stdout, _) =
        super::exec::run_command(&service, secret, &literal).expect("env echo should run");
    assert!(stdout.contains("$OPENAI_API_KEY"));
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
        if matches!(
            preset.id.as_str(),
            "custom"
                | "aws"
                | "supabase"
                | "tavily"
                | "jules"
                | "pix"
                | "pixabay"
                | "exa"
                | "bluesky"
        ) {
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
    assert!(sub
        .iter()
        .any(|(n, v)| n == "authorization" && v == "Bearer sb-secret"));
    // GitLab uses its private-token header with no prefix.
    let gl = build_headers("gitlab", "glpat-x", None, None, None);
    assert!(gl
        .iter()
        .any(|(n, v)| n == "private-token" && v == "glpat-x"));
    // AWS declares companion env fields for the modal.
    let aws = get_preset("aws");
    assert!(aws.meta_fields.iter().any(|f| f.key == "AWS_ACCESS_KEY_ID"));
    assert!(aws
        .meta_fields
        .iter()
        .any(|f| f.key == "AWS_DEFAULT_REGION"));
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
    let legacy: Vec<VaultKey> =
        serde_json::from_str(r#"[{"id":"key-0","secret":"abc"}]"#).expect("legacy key");
    assert_eq!(legacy[0].tag, None);
}

#[test]
fn new_preset_auth_schemas_match_canonical_specs() {
    let deepgram = build_headers("deepgram", "dg-key", None, None, None);
    assert!(deepgram
        .iter()
        .any(|(n, v)| n == "authorization" && v == "Token dg-key"));
    assert_eq!(get_preset("deepgram").env_var, "DEEPGRAM_API_KEY");
    let unsplash = build_headers("unsplash", "pub-key", None, None, None);
    assert!(unsplash
        .iter()
        .any(|(n, v)| n == "authorization" && v == "Client-ID pub-key"));
    assert_eq!(get_preset("unsplash").env_var, "UNSPLASH_ACCESS_KEY");
    let exa = build_headers("exa", "ex-key", None, None, None);
    assert!(exa.iter().any(|(n, v)| n == "x-api-key" && v == "ex-key"));
    assert_eq!(get_preset("exa").env_var, "EXA_API_KEY");
    let stripe = build_headers("stripe", "sk_live", None, None, None);
    assert!(stripe
        .iter()
        .any(|(n, v)| n == "authorization" && v == "Bearer sk_live"));
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
    assert_eq!(
        resolve_saved_tag(cleared.as_ref(), Some("old".to_string())),
        None
    );
    let blank = Some(Some("  ".to_string()));
    assert_eq!(
        resolve_saved_tag(blank.as_ref(), Some("old".to_string())),
        None
    );
    // No entry (no array / short array) keeps the previous tag.
    assert_eq!(
        resolve_saved_tag(None, Some("old".to_string())),
        Some("old".to_string())
    );
    assert_eq!(resolve_saved_tag(None, None), None);
}

#[test]
fn vault_exec_drains_large_output_without_deadlock() {
    // Regression: piped stdout/stderr must be drained while the child runs.
    // ~100KB per stream saturates the 64KB OS pipe buffer under the old
    // try_wait-then-read loop and deadlocked until the 60s timeout.
    let service = test_service("openai", &["sk-secret-9999"]);
    let secret = "sk-secret-9999";
    // No shell is available, so the flood is many `echo` arguments: a single
    // 200KB argument would exceed the OS per-argument limit (E2BIG), while
    // one hundred 2KB arguments total ~200KB and stay well under it.
    let mut flood_argv = vec![serde_json::Value::String("echo".to_string())];
    flood_argv.extend((0..100).map(|_| serde_json::Value::String("x".repeat(2_000))));
    let flood = serde_json::Value::Array(flood_argv);
    let start = std::time::Instant::now();
    let (code, stdout, _) =
        super::exec::run_command(&service, secret, &flood).expect("large output should complete");
    assert_eq!(code, 0);
    assert!(stdout.len() > 64 * 1024, "stdout should exceed pipe buffer");
    assert!(
        start.elapsed() < std::time::Duration::from_secs(60),
        "must not hit the 60s timeout"
    );
}

#[test]
fn vault_exec_captures_stderr_without_shell() {
    // `printf` with a bad conversion writes to stderr and exits nonzero —
    // deterministically, with no network and no shell redirection.
    let service = test_service("openai", &["sk-secret-9999"]);
    let secret = "sk-secret-9999";
    let (code, _, stderr) = super::exec::run_command(
        &service,
        secret,
        &serde_json::json!(["printf", "%d", "not-a-number"]),
    )
    .expect("printf should run");
    assert_ne!(code, 0);
    assert!(!stderr.is_empty(), "stderr must be captured");
    assert!(!stderr.contains(secret));
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

#[test]
fn concurrent_vault_updates_lose_nothing() {
    // TOCTOU regression: stale-snapshot saves and the static tmp name used
    // to drop concurrent writes. 8 threads × 25 bumps must sum exactly.
    use super::atomic::update_vault;
    use super::store::load_vault;
    use crate::paths::OrquestraPaths;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "orquestra-vault-race-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("scratch dir");
    let paths = OrquestraPaths {
        root: root.clone(),
        vault: root.clone(),
        active: root.clone(),
    };
    let mut handles = Vec::new();
    for _ in 0..8 {
        let paths = paths.clone();
        handles.push(std::thread::spawn(move || {
            for _ in 0..25 {
                update_vault(&paths, |vault| {
                    let service = vault
                        .services
                        .entry("ticker".to_string())
                        .or_insert_with(|| test_service("custom", &["k"]));
                    if service.id != "ticker" {
                        service.id = "ticker".to_string();
                    }
                    let count: u64 = service
                        .meta
                        .get("count")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    service
                        .meta
                        .insert("count".to_string(), (count + 1).to_string());
                    Ok(())
                })
                .expect("bump");
            }
        }));
    }
    for handle in handles {
        handle.join().expect("worker");
    }
    let vault = load_vault(&paths).expect("reload");
    let count: u64 = vault.services["ticker"].meta["count"]
        .parse()
        .expect("count");
    assert_eq!(count, 200, "every concurrent bump must survive");
    let leftovers: Vec<_> = std::fs::read_dir(&root)
        .expect("scan")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("vault.json.tmp")
        })
        .collect();
    assert!(
        leftovers.is_empty(),
        "unique tmp names must leave nothing behind: {leftovers:?}"
    );
    std::fs::remove_dir_all(&root).expect("cleanup");
}

/// The shared client must be one instance: same pool, same TLS sessions.
#[test]
fn http_client_is_shared_across_calls() {
    let first = super::http::shared_client().expect("client builds") as *const _;
    let second = super::http::shared_client().expect("client builds") as *const _;
    assert_eq!(first, second, "every request must reuse the pool");
}

/// Serve one canned HTTP response on loopback and return its URL. The
/// handler reads the request first, then writes the bytes and closes —
/// the standard order, so the client never observes a close-before-request.
fn loopback_serve(response: Vec<u8>) -> String {
    use std::io::{Read as _, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback listener");
    let port = listener.local_addr().expect("port").port();
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf);
        let _ = stream.write_all(&response);
    });
    format!("http://127.0.0.1:{port}/v1/models")
}

/// Drive the async helper on Tauri's runtime: the crate has no runtime of
/// its own in production either (Tauri owns it), so tests reuse that same
/// reactor instead of adding a tokio dev-dependency for a stub executor.
fn block_on<F>(future: F) -> F::Output
where
    F: std::future::Future,
{
    tauri::async_runtime::block_on(future)
}

/// A small body flows through the shared client untouched.
#[test]
fn http_small_body_returns_through_shared_client() {
    let body = r#"{"models":[]}"#;
    let url = loopback_serve(
        format!(
            "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes(),
    );
    let (status, _, text) =
        block_on(super::http::execute_once("GET", &url, &[], None)).expect("small body must pass");
    assert_eq!(status, 200);
    assert_eq!(text, body);
}

/// An 11MB `Content-Length` is refused from the headers alone: no 11MB
/// allocation, no wait for a body that never needs to arrive.
#[test]
fn http_huge_body_is_refused_before_reading() {
    let url = loopback_serve(
        b"HTTP/1.0 200 OK\r\nContent-Length: 11534336\r\nConnection: close\r\n\r\nx".to_vec(),
    );
    let error = block_on(super::http::execute_once("GET", &url, &[], None))
        .expect_err("11MB body must be refused");
    assert!(error.contains("cap"), "unexpected error: {error}");
}

/// A no-op `update_vault` must not touch the disk: health pings re-record
/// the same state on every request, and the rewrite + fsync used to run
/// every time. mtime is the observable; the sleep clears even 1s-granular
/// filesystems so a missed skip cannot hide.
#[test]
fn noop_vault_update_skips_disk_write() {
    use super::atomic::update_vault;
    use super::store::{load_vault, vault_path, VaultFile};
    use crate::paths::OrquestraPaths;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "orquestra-vault-skip-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("scratch dir");
    let paths = OrquestraPaths {
        root: root.clone(),
        vault: root.clone(),
        active: root.clone(),
    };
    update_vault(&paths, |vault| {
        vault
            .services
            .insert("svc".to_string(), test_service("openai", &["sk-noop"]));
        Ok(())
    })
    .expect("seed");
    let file = vault_path(&paths);
    let before = std::fs::metadata(&file)
        .expect("vault file")
        .modified()
        .expect("mtime");
    std::thread::sleep(std::time::Duration::from_millis(1100));

    update_vault(&paths, |_: &mut VaultFile| Ok(())).expect("no-op");
    let after_noop = std::fs::metadata(&file)
        .expect("vault file")
        .modified()
        .expect("mtime");
    assert_eq!(
        before, after_noop,
        "a no-op update must not rewrite the vault file"
    );

    // The test can tell: a real mutation must still advance the mtime.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    update_vault(&paths, |vault| {
        vault
            .services
            .get_mut("svc")
            .expect("seeded")
            .meta
            .insert("touched".to_string(), "1".to_string());
        Ok(())
    })
    .expect("mutate");
    let after_write = std::fs::metadata(&file)
        .expect("vault file")
        .modified()
        .expect("mtime");
    assert!(
        after_write > after_noop,
        "a real mutation must still hit the disk"
    );
    let reloaded = load_vault(&paths).expect("reload");
    assert!(reloaded.services.contains_key("svc"));
    std::fs::remove_dir_all(&root).expect("cleanup");
}
