use super::*;
use serde_json::Value;

#[test]
fn interpolation_resolves_process_then_dotenv_and_preserves_missing_variables() {
    let dotenv = HashMap::from([
        ("FROM_FILE".to_string(), "file-value".to_string()),
        ("OVERRIDE".to_string(), "dotenv-value".to_string()),
    ]);
    let process = |name: &str| match name {
        "OVERRIDE" => Ok("process-value".to_string()),
        _ => Err(std::env::VarError::NotPresent),
    };
    assert_eq!(
        interpolate_with(
            "${OVERRIDE} $FROM_FILE ${MISSING} ${MISSING:-fallback}",
            &dotenv,
            &process
        ),
        "process-value file-value ${MISSING} fallback"
    );
}

#[test]
fn interpolation_honors_escaped_dollars_and_malformed_expressions() {
    let process =
        |_: &str| -> Result<String, std::env::VarError> { Err(std::env::VarError::NotPresent) };
    assert_eq!(
        interpolate_with(r"\$TOKEN ${BROKEN", &HashMap::new(), &process),
        "$TOKEN ${BROKEN"
    );
}

#[test]
fn dotenv_parser_reads_values_without_mutating_process_environment() {
    let parsed = parse_dotenv("export TOKEN='file-secret'\nPLAIN=value # note\n# ignored\n");
    assert_eq!(parsed["TOKEN"], "file-secret");
    assert_eq!(parsed["PLAIN"], "value");
    assert!(!parsed.contains_key("ignored"));
}

#[test]
fn dotenv_strips_quotes_when_a_comment_follows() {
    let parsed = parse_dotenv(
        "export TOKEN='file-secret' # production\nQUOTED=\"abc def\" # note\nHASH='file # secret'\n",
    );
    assert_eq!(parsed["TOKEN"], "file-secret");
    assert_eq!(parsed["QUOTED"], "abc def");
    assert_eq!(parsed["HASH"], "file # secret");
}

#[test]
fn secret_masking_handles_empty_short_long_and_template_values() {
    assert_eq!(mask_secret(""), "");
    assert_eq!(mask_secret("short"), "********");
    assert_eq!(mask_secret("sk-123456789"), "****6789");
    assert_eq!(mask_value("API_TOKEN", "raw"), "********");
    assert_eq!(mask_value("API_TOKEN", "${TOKEN}"), "${TOKEN}");
}

#[test]
fn url_secrets_are_masked_when_the_query_key_is_uppercase() {
    let mut server = super::super::McpServer {
        id: "remote".to_string(),
        name: "remote".to_string(),
        description: String::new(),
        command: String::new(),
        args: Vec::new(),
        env: HashMap::new(),
        url: Some("https://mcp.example/sse?API_KEY=sk-live-abcdefghij".to_string()),
        headers: HashMap::new(),
        global_enabled: false,
        enabled_harnesses: Vec::new(),
        official: false,
    };
    mask_server(&mut server);
    let url = server.url.expect("url");
    assert!(
        !url.contains("sk-live"),
        "uppercase query key must not leak"
    );
    assert!(url.starts_with("****"));
}

#[test]
fn inserting_a_flag_does_not_store_the_mask_as_the_secret() {
    let previous = vec!["--token=supersecretvalue".to_string()];
    let mut shifted = vec!["--verbose".to_string(), "--token=****alue".to_string()];
    restore_masked_args(&mut shifted, &previous);
    assert_eq!(
        shifted,
        vec![
            "--verbose".to_string(),
            "--token=supersecretvalue".to_string(),
        ]
    );

    let mut replaced = vec!["--token=brand-new-secret".to_string()];
    restore_masked_args(&mut replaced, &previous);
    assert_eq!(replaced, vec!["--token=brand-new-secret".to_string()]);
}

#[test]
fn deployment_projection_resolves_templates_without_mutating_the_catalog_value() {
    let original = "${ORQUESTRA_MCP_TEST_SECRET_837491}";
    let server = super::super::McpServer {
        id: "test".to_string(),
        name: "test".to_string(),
        description: String::new(),
        command: "tool".to_string(),
        args: vec!["--token=${ORQUESTRA_MCP_TEST_SECRET_837491}".to_string()],
        env: HashMap::from([("TOKEN".to_string(), original.to_string())]),
        url: Some("https://${MCP_TEST_HOST}/mcp".to_string()),
        headers: HashMap::from([(
            "Authorization".to_string(),
            "Bearer ${ORQUESTRA_MCP_TEST_SECRET_837491}".to_string(),
        )]),
        global_enabled: true,
        enabled_harnesses: Vec::new(),
        official: false,
    };
    let dotenv = HashMap::from([
        (
            "ORQUESTRA_MCP_TEST_SECRET_837491".to_string(),
            "resolved".to_string(),
        ),
        ("MCP_TEST_HOST".to_string(), "mcp.example.test".to_string()),
    ]);
    let projected = server.resolved(&dotenv);

    assert_eq!(projected.env["TOKEN"], "resolved");
    assert_eq!(projected.args, vec!["--token=resolved"]);
    assert_eq!(
        projected.url.as_deref(),
        Some("https://mcp.example.test/mcp")
    );
    assert_eq!(projected.headers["Authorization"], "Bearer resolved");
    assert_eq!(server.env["TOKEN"], original);

    let mut local = projected.clone();
    local.url = None;
    let mut remote = projected;
    remote.id = "remote".to_string();
    remote.args.clear();
    remote.env.clear();
    let rendered = super::super::serializers::render_mcp_servers(
        "",
        &[local, remote],
        &Default::default(),
        None,
    )
    .unwrap();
    let rendered: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(rendered["mcpServers"]["test"]["env"]["TOKEN"], "resolved");
    assert_eq!(
        rendered["mcpServers"]["test"]["args"][0],
        "--token=resolved"
    );
    assert_eq!(
        rendered["mcpServers"]["remote"]["url"],
        "https://mcp.example.test/mcp"
    );
    assert_eq!(
        rendered["mcpServers"]["test"]["headers"]["Authorization"],
        "Bearer resolved"
    );

    let serialized: Value = serde_json::to_value(&server).unwrap();
    assert_eq!(serialized["env"]["TOKEN"], original);
    assert_eq!(serialized["url"], "https://${MCP_TEST_HOST}/mcp");
    assert_eq!(
        serialized["headers"]["Authorization"],
        "Bearer ${ORQUESTRA_MCP_TEST_SECRET_837491}"
    );
}
