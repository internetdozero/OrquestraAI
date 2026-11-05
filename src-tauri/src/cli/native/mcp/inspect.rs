//! Detailed server inspection command.

use super::super::{parse_options, rendered, usage, CliError};
use crate::mcp::{list_mcps, list_mcps_masked};

pub(super) fn inspect(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| {
            usage("usage: orquestra mcp inspect <id> [--reveal] [--json]".to_string())
        })?;

    let options = parse_options(&args[1..], &[], &["--reveal", "--json"])?;

    let servers = if options.has("--reveal") {
        list_mcps().map_err(CliError::Operation)?
    } else {
        list_mcps_masked().map_err(CliError::Operation)?
    };

    let server = servers
        .into_iter()
        .find(|s| s.id == *id)
        .ok_or_else(|| CliError::Operation(format!("unknown MCP server: {id}")))?;

    let status_str = if server.active_anywhere() {
        if server.global_enabled && !server.enabled_harnesses.is_empty() {
            format!(
                "active (global, {} harnesses)",
                server.enabled_harnesses.len()
            )
        } else if server.global_enabled {
            "active (global)".to_string()
        } else {
            format!("active ({} harnesses)", server.enabled_harnesses.len())
        }
    } else {
        "inactive".to_string()
    };

    let mut env_lines = Vec::new();
    if server.env.is_empty() {
        env_lines.push("  (none)".to_string());
    } else {
        let mut sorted_keys: Vec<_> = server.env.keys().collect();
        sorted_keys.sort();
        for key in sorted_keys {
            env_lines.push(format!("  {}: {}", key, server.env[key]));
        }
    }

    let mut header_lines = Vec::new();
    if !server.headers.is_empty() {
        let mut sorted_keys: Vec<_> = server.headers.keys().collect();
        sorted_keys.sort();
        for key in sorted_keys {
            header_lines.push(format!("  {}: {}", key, server.headers[key]));
        }
    }

    let harnesses_str = if server.enabled_harnesses.is_empty() {
        "none".to_string()
    } else {
        server.enabled_harnesses.join(", ")
    };

    let url_str = server.url.as_deref().unwrap_or("(none)");
    let args_str = if server.args.is_empty() {
        "(none)".to_string()
    } else {
        server.args.join(" ")
    };

    let mut human = format!(
        "ID:          {}\n\
         Name:        {}\n\
         Description: {}\n\
         Official:    {}\n\
         Command:     {}\n\
         Args:        {}\n\
         URL:         {}\n\
         Environment:\n{}\n\
         Scopes:\n\
           Global:     {}\n\
           Harnesses:  {}\n\
         Status:       {}",
        server.id,
        server.name,
        if server.description.is_empty() {
            "(none)"
        } else {
            &server.description
        },
        server.official,
        server.command,
        args_str,
        url_str,
        env_lines.join("\n"),
        server.global_enabled,
        harnesses_str,
        status_str
    );

    if !header_lines.is_empty() {
        human.push_str(&format!("\nHeaders:\n{}", header_lines.join("\n")));
    }

    rendered(&server, options.has("--json"), human)
}
