//! Integration coverage for the blind `vault_exec` path at the IPC-command
//! level: service setup, small and >64KB output, secret scrubbing, and every
//! rejection class. Runs against a scratch `HOME`, never the real vault.

mod common;

use orquestra_ai_lib::vault::{vault_delete_service, vault_exec, vault_save_service};

const SECRET: &str = "sk-it-lifecycle-abcdef123456";

fn save_test_service() {
    vault_save_service(
        "it-service".to_string(),
        "Integration Test".to_string(),
        "custom".to_string(),
        None,
        vec![SECRET.to_string()],
        None,
        None,
        None,
        None,
        None,
    )
    .expect("save service");
}

fn array(parts: &[&str]) -> serde_json::Value {
    serde_json::Value::Array(
        parts
            .iter()
            .map(|part| serde_json::Value::String(part.to_string()))
            .collect(),
    )
}

#[test]
fn small_command_returns_scrubbed_output() {
    common::with_temp_home("exec-small", |_| {
        save_test_service();

        let result = vault_exec("it-service".to_string(), array(&["echo", "hello-exec"]))
            .expect("echo runs");
        assert_eq!(result.code, 0);
        assert!(result.stdout.contains("hello-exec"));
        assert_eq!(result.used_key_index, 0);

        // The secret crosses into the child env only; any echo of it comes
        // back masked, never verbatim.
        let result =
            vault_exec("it-service".to_string(), array(&["echo", SECRET])).expect("echo runs");
        assert!(
            !result.stdout.contains(SECRET),
            "secret leaked: {}",
            result.stdout
        );
        assert!(result.stdout.contains("[REDACTED]"));

        vault_delete_service("it-service".to_string()).expect("delete service");
    });
}

#[test]
fn large_output_drains_without_deadlock() {
    common::with_temp_home("exec-large", |_| {
        save_test_service();

        // ~200KB on stdout: past the 64KB OS pipe buffer that deadlocked the
        // old try_wait-then-read loop. One 2KB argument per slot stays under
        // the OS per-argument limit while the total exceeds the buffer.
        let mut parts = vec!["echo".to_string()];
        parts.extend((0..100).map(|_| "x".repeat(2_000)));
        let command =
            serde_json::Value::Array(parts.into_iter().map(serde_json::Value::String).collect());
        let result = vault_exec("it-service".to_string(), command).expect("large echo runs");
        assert_eq!(result.code, 0);
        assert!(
            result.stdout.len() > 64 * 1024,
            "stdout must exceed the pipe buffer (got {})",
            result.stdout.len()
        );
        assert!(!result.stdout.contains(SECRET));
    });
}

#[test]
fn hostile_and_unknown_commands_fail_closed() {
    common::with_temp_home("exec-reject", |_| {
        save_test_service();

        // Shell strings never reach an interpreter.
        assert!(vault_exec(
            "it-service".to_string(),
            serde_json::Value::String("echo hello".to_string())
        )
        .is_err());
        // Anything off the allowlist is refused before the OS loader.
        assert!(vault_exec("it-service".to_string(), array(&["sh", "-c", "echo hi"])).is_err());
        // Paths smuggled as the program are refused too.
        assert!(vault_exec("it-service".to_string(), array(&["/bin/echo", "hi"])).is_err());
        // Non-string array items cannot smuggle types past validation.
        assert!(vault_exec(
            "it-service".to_string(),
            serde_json::Value::Array(vec![serde_json::json!("echo"), serde_json::json!(7)])
        )
        .is_err());
        // Unknown services and post-delete services error, never run bare.
        assert!(vault_exec("missing".to_string(), array(&["echo", "hi"])).is_err());
        vault_delete_service("it-service".to_string()).expect("delete service");
        assert!(vault_exec("it-service".to_string(), array(&["echo", "hi"])).is_err());
    });
}
