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

/// Executables IPC callers may run. Bare names only, resolved through the
/// trusted session `PATH` — never a shell, so metacharacters, pipes,
/// redirections and `$VAR`/backtick/`$()` expansion cannot trigger.
///
/// Deliberately minimal: provider CLIs agents need for authenticated calls,
/// plus `echo`/`printf` for smoke tests. Notably absent: shells (`sh`,
/// `bash`), script runtimes (`python*`, `node`, `perl`, `ruby`), downloaders
/// (`curl`, `wget`), `ssh`, and `git`/`find`/`xargs` (all have exec escapes).
/// Extend only in code review — there is intentionally no runtime knob, so a
/// compromised webview cannot widen this list through IPC.
const ALLOWED_BINARIES: &[&str] = &[
    "aws", "az", "doctl", "echo", "gcloud", "gh", "kubectl", "printf", "stripe",
    "terraform",
];

/// Hard caps so one IPC call cannot exhaust memory or the argument vector.
const MAX_ARGV_LEN: usize = 128;
const MAX_ARG_CHARS: usize = 1_000_000;

/// Array form ONLY: execute `argv[0]` with `argv[1..]` directly, no shell.
/// Rejects anything outside [`ALLOWED_BINARIES`], any path separator, NUL
/// bytes (which would panic `Command`), empty parts and oversized input.
pub(super) fn run_argv(
    service: &VaultService,
    secret: &str,
    argv: &[String],
) -> Result<(i32, String, String), String> {
    validate_argv(argv)?;
    let env = child_env(service, secret);
    let mut child = std::process::Command::new(&argv[0]);
    child.args(&argv[1..]);
    for (name, value) in &env {
        child.env(name, value);
    }
    wait_scrubbed(child, secret, &env)
}

fn validate_argv(argv: &[String]) -> Result<(), String> {
    if argv.is_empty() || argv.len() > MAX_ARGV_LEN {
        return Err("vault exec needs a non-empty command array".to_string());
    }
    let program = &argv[0];
    // Bare allowlisted name only: no directories, no traversal, no NUL.
    // The check runs before PATH resolution, so `./evil`, `/bin/evil` and
    // `C:\evil` never reach the OS loader even if such a file existed.
    if program.contains(['/', '\\', '\0']) {
        return Err(format!("vault exec program {program:?} must be a bare name"));
    }
    if !ALLOWED_BINARIES.contains(&program.as_str()) {
        return Err(format!(
            "vault exec program {program:?} is not on the allowlist"
        ));
    }
    for part in argv {
        if part.is_empty() || part.contains('\0') || part.len() > MAX_ARG_CHARS {
            return Err("vault exec arguments must be non-empty NUL-free text".to_string());
        }
    }
    Ok(())
}

/// Accept ONLY the string-array form. Shell strings were removed: any `sh -c`
/// path lets IPC input reach a shell, which is unauditable by construction.
/// The Tauri signature keeps `command: Value` so the wire shape is stable;
/// string payloads are now rejected at runtime with a dedicated error.
pub(super) fn run_command(
    service: &VaultService,
    secret: &str,
    command: &serde_json::Value,
) -> Result<(i32, String, String), String> {
    if let Some(text) = command.as_str() {
        return Err(format!(
            "vault exec rejects shell strings ({text:?}): pass a string array of an allowlisted program instead"
        ));
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
    Err("vault exec command must be a string array".to_string())
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
    // Drain pipes on dedicated threads while the child runs. Without this,
    // >64KB of output saturates the OS pipe buffer: the child blocks on
    // write while the parent blocks in try_wait — a mutual deadlock.
    // Pipes are taken up front so the reader threads own only the pipe
    // handles and the parent keeps the `Child` for try_wait/kill.
    let stdout_pipe = spawned.stdout.take();
    let stderr_pipe = spawned.stderr.take();
    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stdout_pipe {
            use std::io::Read as _;
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stderr_pipe {
            use std::io::Read as _;
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let start = std::time::Instant::now();
    loop {
        match spawned.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);
                // Child exited and closed its write ends, so the readers see
                // EOF and finish; join to collect everything they drained.
                let stdout_bytes = stdout_handle.join().unwrap_or_default();
                let stderr_bytes = stderr_handle.join().unwrap_or_default();
                let stdout = String::from_utf8_lossy(&stdout_bytes).into_owned();
                let stderr = String::from_utf8_lossy(&stderr_bytes).into_owned();
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
                    // Reap the child so the pipe write ends close and the
                    // reader threads observe EOF instead of blocking join.
                    let _ = spawned.wait();
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return Err("vault exec timed out after 60s".to_string());
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                let _ = spawned.kill();
                let _ = spawned.wait();
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
                return Err(format!("vault exec wait failed: {error}"));
            }
        }
    }
}
