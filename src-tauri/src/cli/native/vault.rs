//! `orquestra vault …` — the blind vault for scripts and the MCP control
//! plane, without a window.
//!
//! The `orquestra-core` MCP server used to read `vault.json` off disk; since
//! the alpha.5 at-rest encryption that file is a sealed envelope, so the MCP
//! delegates here instead: the secrets stay inside the Rust backend and only
//! masked metadata or scrubbed results reach stdout.

use std::collections::HashMap;

use super::{parse_options, rendered, usage, CliError};

const USAGE: &str = "usage: orquestra vault <list|set-enabled|http|exec> [options]";

pub(super) fn run(args: &[String]) -> Result<String, CliError> {
    match args.first().map(String::as_str) {
        Some("list") => list(&args[1..]),
        Some("set-enabled") => set_enabled(&args[1..]),
        Some("http") => http(&args[1..]),
        Some("exec") => exec(&args[1..]),
        _ => Err(usage(USAGE.to_string())),
    }
}

fn paths() -> Result<orquestra_core::paths::OrquestraPaths, CliError> {
    orquestra_core::paths::OrquestraPaths::resolve().map_err(|error| CliError::Operation(error.to_string()))
}

/// Masked services only: ids, labels, kill-switch, health, key counts.
fn list(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(args, &[], &["--json"])?;
    let services =
        orquestra_vault::vault::service::list_services(&paths()?).map_err(CliError::Operation)?;
    let human = if services.is_empty() {
        "No credential services.".to_string()
    } else {
        services
            .iter()
            .map(|service| {
                let state = if service.enabled { "enabled" } else { "disabled" };
                format!(
                    "{} ({}): {state}, {}, {} keys",
                    service.id, service.label, service.health, service.total_keys
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    rendered(&services, options.has("--json"), human)
}

/// Flip the operator kill-switch; the blind proxy refuses disabled services.
fn set_enabled(args: &[String]) -> Result<String, CliError> {
    // `vault set-enabled <id> <on|off> [--json]`; parse by hand because the
    // pair is positional and `--json` is an optional trailer.
    let (id, state, as_json) = match args {
        [id, state] => (id.as_str(), state.as_str(), false),
        [id, state, flag] if flag == "--json" => (id.as_str(), state.as_str(), true),
        _ => {
            return Err(usage(
                "usage: orquestra vault set-enabled <id> <on|off> [--json]".to_string(),
            ));
        }
    };
    let enabled = match state {
        "on" | "true" | "enable" => true,
        "off" | "false" | "disable" => false,
        other => return Err(usage(format!("expected on|off, got {other}"))),
    };
    let service =
        orquestra_vault::vault::service::set_service_enabled(&paths()?, id, enabled)
            .map_err(CliError::Operation)?;
    let human = format!(
        "Service {} {}.",
        service.label,
        if service.enabled { "enabled" } else { "disabled" }
    );
    rendered(&service, as_json, human)
}

/// Blind HTTP: inject the active key, fail over, scrub secrets from output.
fn http(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &[
            "--service",
            "--endpoint",
            "--method",
            "--headers",
            "--body",
            "--key-tag",
        ],
        &["--json"],
    )?;
    let service = options.required("--service")?;
    let endpoint = options.required("--endpoint")?;
    let method = options.one("--method")?;
    let headers: Option<HashMap<String, String>> = options
        .one("--headers")?
        .map(|raw| {
            serde_json::from_str(&raw)
                .map_err(|_| usage("--headers must be a JSON object".to_string()))
        })
        .transpose()?;
    let body = options.one("--body")?;
    let key_tag = options.one("--key-tag")?;
    let result = tauri::async_runtime::block_on(orquestra_vault::vault::proxy::request(
        service, endpoint, method, headers, body, key_tag,
    ))
    .map_err(CliError::Operation)?;
    let pretty = serde_json::to_string_pretty(&result)
        .map_err(|error| CliError::Operation(format!("JSON serialization failed: {error}")))?;
    rendered(&result, options.has("--json"), pretty)
}

/// Blind exec: `argv[0]` must be an allowlisted bare binary, no shell.
fn exec(args: &[String]) -> Result<String, CliError> {
    // `vault exec <service> [--] <argv…>`; `--` is accepted and skipped so
    // generated callsites can separate flags from the child command.
    let mut parts: Vec<&str> = args.iter().map(String::as_str).collect();
    let Some(service) = parts.first().copied() else {
        return Err(usage("usage: orquestra vault exec <service> [--] <argv…>".to_string()));
    };
    parts.remove(0);
    if parts.first() == Some(&"--") {
        parts.remove(0);
    }
    if service.starts_with('-') || parts.is_empty() {
        return Err(usage("usage: orquestra vault exec <service> [--] <argv…>".to_string()));
    }
    let command =
        serde_json::to_value(&parts).map_err(|error| CliError::Operation(error.to_string()))?;
    let result = orquestra_vault::vault::service::execute(&paths()?, service, &command)
        .map_err(CliError::Operation)?;
    let pretty = serde_json::to_string_pretty(&result)
        .map_err(|error| CliError::Operation(format!("JSON serialization failed: {error}")))?;
    rendered(&result, true, pretty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_subcommands_show_usage() {
        assert!(matches!(run(&[]), Err(CliError::Usage(_))));
        assert!(matches!(
            run(&["bogus".to_string()]),
            Err(CliError::Usage(_))
        ));
    }

    #[test]
    fn set_enabled_rejects_bad_shapes_before_touching_disk() {
        for args in [
            vec![],
            vec!["svc".to_string()],
            vec!["svc".to_string(), "maybe".to_string()],
            vec![
                "svc".to_string(),
                "on".to_string(),
                "extra".to_string(),
                "junk".to_string(),
            ],
        ] {
            assert!(matches!(set_enabled(&args), Err(CliError::Usage(_))));
        }
        // Unknown services fail closed as an operation error, not usage.
        assert!(matches!(
            set_enabled(&["nope".to_string(), "off".to_string()]),
            Err(CliError::Operation(_))
        ));
    }

    #[test]
    fn exec_rejects_empty_argv_before_touching_disk() {
        assert!(matches!(exec(&[]), Err(CliError::Usage(_))));
        assert!(matches!(
            exec(&["svc".to_string()]),
            Err(CliError::Usage(_))
        ));
    }

    #[test]
    fn http_requires_service_and_endpoint() {
        // Missing options must be usage, never disk IO.
        assert!(matches!(http(&[]), Err(CliError::Usage(_))));
        assert!(matches!(
            http(&["--service".to_string(), "svc".to_string()]),
            Err(CliError::Usage(_))
        ));
    }
}
