//! MCP Protocol interaction: test-connection, tools-list, and call-tool.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::super::{parse_options, parse_u32, rendered, usage, CliError};
use crate::mcp::{list_mcps, parse_dotenv, McpServer};
use crate::paths::OrquestraPaths;

fn get_resolved_server(id: &str) -> Result<McpServer, CliError> {
    let servers = list_mcps().map_err(CliError::Operation)?;
    let server = servers
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| CliError::Operation(format!("unknown MCP server: {id}")))?;

    let paths = OrquestraPaths::resolve().map_err(|e| CliError::Operation(e.to_string()))?;
    let dotenv_path = paths.root.join(".env");
    let dotenv = match fs::read_to_string(&dotenv_path) {
        Ok(content) => parse_dotenv(&content),
        Err(_) => HashMap::new(),
    };

    Ok(server.resolved(&dotenv))
}

pub(super) fn test_connection(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra mcp test-connection <id> [--timeout <ms>] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &["--timeout"], &["--json"])?;
    let timeout_ms = parse_u32(&options, "--timeout")?.unwrap_or(5000) as u64;

    let server = get_resolved_server(id)?;
    let start = Instant::now();

    if let Some(url) = &server.url {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .map_err(|e| CliError::Operation(format!("HTTP client error: {e}")))?;

        let mut req = client.get(url);
        for (k, v) in &server.headers {
            req = req.header(k, v);
        }

        let resp = req.send().map_err(|e| CliError::Operation(format!("connection failed: {e}")))?;
        let latency_ms = start.elapsed().as_millis();

        let report = serde_json::json!({
            "id": server.id,
            "status": "Healthy",
            "type": "http",
            "statusCode": resp.status().as_u16(),
            "roundtripLatencyMs": latency_ms,
        });

        let human = format!(
            "Server '{}': OK\n  Type: HTTP / SSE\n  Status Code: {}\n  Roundtrip Latency: {}ms\n  Status: Healthy",
            server.id, resp.status(), latency_ms
        );
        return rendered(&report, options.has("--json"), human);
    }

    // stdio probe
    let init_resp = run_stdio_exchange(&server, None, 1, timeout_ms)?;
    let latency_ms = start.elapsed().as_millis();

    let server_info = init_resp
        .get("result")
        .and_then(|r| r.get("serverInfo"));
    let server_name = server_info
        .and_then(|i| i.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let server_ver = server_info
        .and_then(|i| i.get("version"))
        .and_then(Value::as_str)
        .unwrap_or("0.0.0");
    let protocol_ver = init_resp
        .get("result")
        .and_then(|r| r.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or("2024-11-05");

    let report = serde_json::json!({
        "id": server.id,
        "status": "Healthy",
        "protocolVersion": protocol_ver,
        "serverImplementation": format!("{server_name} v{server_ver}"),
        "roundtripLatencyMs": latency_ms,
    });

    let human = format!(
        "Server '{}': OK\n  Protocol Version: {}\n  Server Implementation: {} v{}\n  Roundtrip Latency: {}ms\n  Status: Healthy",
        server.id, protocol_ver, server_name, server_ver, latency_ms
    );

    rendered(&report, options.has("--json"), human)
}

pub(super) fn tools_list(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra mcp tools-list <id> [--timeout <ms>] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &["--timeout"], &["--json"])?;
    let timeout_ms = parse_u32(&options, "--timeout")?.unwrap_or(5000) as u64;

    let server = get_resolved_server(id)?;
    if server.url.is_some() {
        return Err(CliError::Operation("remote HTTP/SSE tool listing not yet supported via stdio transport".to_string()));
    }

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });

    let resp = run_stdio_exchange(&server, Some(req), 2, timeout_ms)?;
    let result = resp.get("result").cloned().unwrap_or(serde_json::json!({}));
    let empty_tools = Vec::new();
    let tools = result
        .get("tools")
        .and_then(Value::as_array)
        .unwrap_or(&empty_tools);

    let mut lines = Vec::new();
    lines.push(format!("Tools for '{}' ({} tools found):", server.id, tools.len()));
    for tool in tools {
        let name = tool.get("name").and_then(Value::as_str).unwrap_or("unknown");
        let desc = tool.get("description").and_then(Value::as_str).unwrap_or("");
        lines.push(format!("  • {name}: {desc}"));
    }

    rendered(&result, options.has("--json"), lines.join("\n"))
}

pub(super) fn call_tool(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra mcp call-tool <id> <tool-name> [--args <json>] [--timeout <ms>] [--json]".to_string()))?;

    let tool_name = args
        .get(1)
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra mcp call-tool <id> <tool-name> [--args <json>] [--timeout <ms>] [--json]".to_string()))?;

    let options = parse_options(&args[2..], &["--args", "--timeout"], &["--json"])?;
    let timeout_ms = parse_u32(&options, "--timeout")?.unwrap_or(15000) as u64;

    let tool_args: Value = match options.one("--args")? {
        Some(s) => serde_json::from_str(&s)
            .map_err(|e| usage(format!("invalid --args JSON: {e}")))?,
        None => serde_json::json!({}),
    };

    let server = get_resolved_server(id)?;
    if server.url.is_some() {
        return Err(CliError::Operation("remote HTTP/SSE tool invocation not yet supported via stdio transport".to_string()));
    }

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": tool_args
        }
    });

    let resp = run_stdio_exchange(&server, Some(req), 3, timeout_ms)?;

    if let Some(err) = resp.get("error") {
        return Err(CliError::Operation(format!("tool error: {err}")));
    }

    let result = resp.get("result").cloned().unwrap_or(serde_json::json!({}));
    let human = format!(
        "Tool '{tool_name}' succeeded:\n{}",
        serde_json::to_string_pretty(&result).unwrap_or_default()
    );

    rendered(&resp, options.has("--json"), human)
}

fn spawn_server(server: &McpServer) -> Result<Child, CliError> {
    let mut cmd = Command::new(&server.command);
    cmd.args(&server.args)
        .envs(&server.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    cmd.spawn().map_err(|e| {
        CliError::Operation(format!("failed to spawn MCP server '{}' ({}): {e}", server.id, server.command))
    })
}

fn run_stdio_exchange(
    server: &McpServer,
    req: Option<Value>,
    expected_id: i64,
    timeout_ms: u64,
) -> Result<Value, CliError> {
    let mut child = spawn_server(server)?;
    let mut stdin = child.stdin.take().ok_or_else(|| CliError::Operation("failed to open child stdin".to_string()))?;
    let stdout = child.stdout.take().ok_or_else(|| CliError::Operation("failed to open child stdout".to_string()))?;

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        while let Ok(bytes) = reader.read_line(&mut line) {
            if bytes == 0 { break; }
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                    if val.get("id") == Some(&serde_json::json!(expected_id)) {
                        let _ = tx.send(Ok(val));
                        return;
                    }
                }
            }
            line.clear();
        }
        let _ = tx.send(Err(CliError::Operation(format!("server stdout closed before id {expected_id} response"))));
    });

    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "orquestra-cli", "version": "0.1.0" }
        }
    });

    let wire = if let Some(secondary_req) = req {
        let initialized = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        format!(
            "{}\n{}\n{}\n",
            serde_json::to_string(&init_req).unwrap_or_default(),
            serde_json::to_string(&initialized).unwrap_or_default(),
            serde_json::to_string(&secondary_req).unwrap_or_default()
        )
    } else {
        format!("{}\n", serde_json::to_string(&init_req).unwrap_or_default())
    };

    stdin.write_all(wire.as_bytes()).map_err(|e| CliError::Operation(format!("failed to write MCP payload: {e}")))?;
    stdin.flush().ok();

    let res = match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(val) => val,
        Err(_) => {
            let _ = child.kill();
            Err(CliError::Operation(format!("timed out waiting for MCP response ({}ms)", timeout_ms)))
        }
    };
    let _ = child.kill();
    res
}
