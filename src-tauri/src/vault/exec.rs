//! Blind subprocess execution: env injection + output scrubbing.
//!
//! The raw secret is only placed in the child process environment
//! (`child_env`); `stdout`/`stderr` are scrubbed before crossing IPC so a
//! leaked echo can never reach the agent transcript.

use std::collections::HashMap;
use std::time::Duration;

use super::http::scrub_text;
use super::presets::build_exec_env;
use super::store::VaultService;

/// Env pairs for the child process (secret lives here only).
pub fn child_env(service: &VaultService, secret: &str) -> HashMap<String, String> {
    let mut env: HashMap<String, String> =
        build_exec_env(&service.preset, secret, service.custom_env.as_deref())
            .into_iter()
            .collect();
    for (name, value) in &service.meta {
        env.insert(name.clone(), value.clone());
    }
    env
}

/// Scrub every provided secret from command output.
pub fn scrub_output(text: &str, secrets: &[String]) -> String {
    scrub_text(text, secrets)
}

/// Run `command` with blind env vars, 60s timeout, scrubbed output.
/// `command` is executed via `sh -c` so operators can use pipes; the
/// secret itself is never interpolated into the shell string.
/// Run `command` with blind env vars, 60s timeout, scrubbed output.
/// `command` is executed via `sh -c` so operators can use pipes; the
/// secret itself is never interpolated into the shell string.
pub(super) fn run_once(
    service: &VaultService,
    secret: &str,
    command: &str,
) -> Result<(i32, String, String), String> {
    if command.trim().is_empty() {
        return Err("vault exec needs a command".to_string());
    }
    let env = child_env(service, secret);
    let mut child = std::process::Command::new("sh");
    child.arg("-c").arg(command);
    for (name, value) in &env {
        child.env(name, value);
    }
    wait_scrubbed(child, secret, &env)
}

/// Array form: execute `argv[0]` with `argv[1..]` directly (no shell), so
/// callers that already split arguments keep exact quoting.
pub(super) fn run_argv(
    service: &VaultService,
    secret: &str,
    argv: &[String],
) -> Result<(i32, String, String), String> {
    if argv.is_empty() || argv.iter().any(|part| part.trim().is_empty()) {
        return Err("vault exec needs a non-empty command array".to_string());
    }
    let env = child_env(service, secret);
    let mut child = std::process::Command::new(&argv[0]);
    child.args(&argv[1..]);
    for (name, value) in &env {
        child.env(name, value);
    }
    wait_scrubbed(child, secret, &env)
}

/// Accept the spec's `command` union: a shell string or a string array.
pub(super) fn run_command(
    service: &VaultService,
    secret: &str,
    command: &serde_json::Value,
) -> Result<(i32, String, String), String> {
    if let Some(text) = command.as_str() {
        return run_once(service, secret, text);
    }
    if let Some(items) = command.as_array() {
        let mut argv = Vec::with_capacity(items.len());
        for item in items {
            match item.as_str() {
                Some(part) => argv.push(part.to_string()),
                None => {
                    return Err(
                        "vault exec command array must hold strings only".to_string(),
                    )
                }
            }
        }
        return run_argv(service, secret, &argv);
    }
    Err("vault exec command must be a string or string array".to_string())
}

fn wait_scrubbed(
    mut child: std::process::Command,
    secret: &str,
    env: &HashMap<String, String>,
) -> Result<(i32, String, String), String> {
    child.stdout(std::process::Stdio::piped());
    child.stderr(std::process::Stdio::piped());
    let mut spawned = child
        .spawn()
        .map_err(|error| format!("vault exec spawn failed: {error}"))?;
    let start = std::time::Instant::now();
    loop {
        match spawned.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);
                let output = spawned
                    .wait_with_output()
                    .map_err(|error| format!("vault exec wait failed: {error}"))?;
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                let secrets: Vec<String> =
                    std::iter::once(secret.to_string()).chain(env.values().cloned()).collect();
                return Ok((
                    code,
                    scrub_output(&stdout, &secrets),
                    scrub_output(&stderr, &secrets),
                ));
            }
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(60) {
                    let _ = spawned.kill();
                    return Err("vault exec timed out after 60s".to_string());
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(format!("vault exec wait failed: {error}")),
        }
    }
}
