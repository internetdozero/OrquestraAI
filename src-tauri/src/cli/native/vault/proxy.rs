//! Blind HTTP proxy, exec proxy, and connectivity test subcommands.

use std::collections::HashMap;
use super::{parse_key_number, paths};
use super::super::{parse_options, rendered, usage, CliError};

pub(super) fn http(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &[
            "--service",
            "--endpoint",
            "--method",
            "--headers",
            "--body",
            "--key-tag",
            "--key-number",
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
    let key_number = options
        .one("--key-number")?
        .map(|value| parse_key_number(&value))
        .transpose()?;

    let result = tauri::async_runtime::block_on(
        orquestra_vault::vault::proxy::request_with_key_number(
            service, endpoint, method, headers, body, key_tag, key_number,
        ),
    )
    .map_err(CliError::Operation)?;

    let pretty = serde_json::to_string_pretty(&result)
        .map_err(|error| CliError::Operation(format!("JSON serialization failed: {error}")))?;
    rendered(&result, options.has("--json"), pretty)
}

pub(super) fn exec(args: &[String]) -> Result<String, CliError> {
    let mut parts: Vec<&str> = args.iter().map(String::as_str).collect();
    let Some(service) = parts.first().copied() else {
        return Err(usage(
            "usage: orquestra vault proxy-exec <service> [--key-number N] [--] <argv…>".to_string(),
        ));
    };
    parts.remove(0);

    let key_number = if parts.first() == Some(&"--key-number") {
        if parts.len() < 2 {
            return Err(usage("--key-number requires a number".to_string()));
        }
        let value = parts[1];
        parts.drain(..2);
        Some(parse_key_number(value)?)
    } else {
        None
    };

    if parts.first() == Some(&"--") {
        parts.remove(0);
    }
    if service.starts_with('-') || parts.is_empty() {
        return Err(usage(
            "usage: orquestra vault proxy-exec <service> [--key-number N] [--] <argv…>".to_string(),
        ));
    }

    let command = serde_json::to_value(&parts).map_err(|error| CliError::Operation(error.to_string()))?;
    let result = orquestra_vault::vault::service::execute_with_key_number(
        &paths()?,
        service,
        &command,
        key_number,
    )
    .map_err(CliError::Operation)?;

    let pretty = serde_json::to_string_pretty(&result)
        .map_err(|error| CliError::Operation(format!("JSON serialization failed: {error}")))?;
    rendered(&result, true, pretty)
}

pub(super) fn test_conn(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra vault test <id> [--key-number <N>] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &["--key-number"], &["--json"])?;
    let key_index = options
        .one("--key-number")?
        .map(|v| parse_key_number(&v))
        .transpose()?
        .map(|n| n - 1);

    let result = tauri::async_runtime::block_on(
        orquestra_vault::vault::proxy::test_connection(id.to_string(), key_index),
    )
    .map_err(CliError::Operation)?;

    let human = format!(
        "Testing service '{id}' with key #{}:\n  \
         Outcome:   {}\n  \
         Latency:   {}ms\n  \
         Health:    {}\n  \
         Message:   {}",
        key_index.map_or(1, |i| i + 1),
        if result.success {
            "SUCCESS"
        } else if result.inconclusive {
            "INCONCLUSIVE"
        } else {
            "FAILURE"
        },
        result.latency_ms,
        result.health,
        result.message
    );

    rendered(&result, options.has("--json"), human)
}
