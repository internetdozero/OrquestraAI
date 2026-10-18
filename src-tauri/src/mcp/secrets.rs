use std::collections::HashMap;

pub(super) fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["token", "key", "secret", "authorization", "password", "bearer"]
        .iter()
        .any(|marker| key.contains(marker))
}

fn sensitive_value(value: &str) -> bool {
    ["sk-", "ghp_", "cf_"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

pub(super) fn mask_value(key: &str, value: &str) -> String {
    if contains_template(value) {
        return value.to_string();
    }
    if sensitive_key(key) || sensitive_value(value) {
        mask_secret(value)
    } else {
        value.to_string()
    }
}

fn contains_template(value: &str) -> bool {
    value.match_indices('$').any(|(index, _)| {
        let bytes = value.as_bytes();
        if index > 0 && bytes[index - 1] == b'\\' {
            return false;
        }
        bytes.get(index + 1).is_some_and(|byte| {
            matches!(*byte, b'{' | b'_' | b'A'..=b'Z' | b'a'..=b'z')
        })
    })
}

pub fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if value.len() <= 8 {
        return "********".to_string();
    }

    let tail = value.chars().rev().take(4).collect::<String>();
    format!("****{}", tail.chars().rev().collect::<String>())
}

pub(super) fn mask_server(server: &mut super::McpServer) {
    server.env = server
        .env
        .iter()
        .map(|(key, value)| (key.clone(), mask_value(key, value)))
        .collect();
    server.headers = server
        .headers
        .iter()
        .map(|(key, value)| (key.clone(), mask_value(key, value)))
        .collect();

    let mut mask_next_arg = false;
    for arg in &mut server.args {
        if mask_next_arg {
            *arg = mask_value("token", arg);
            mask_next_arg = false;
            continue;
        }
        if let Some((key, value)) = arg.split_once('=') {
            if sensitive_key(key) || sensitive_value(value) {
                *arg = format!("{key}={}", mask_value(key, value));
            }
        } else if sensitive_key(arg) {
            mask_next_arg = true;
        } else if sensitive_value(arg) {
            *arg = mask_secret(arg);
        }
    }

    if let Some(url) = &mut server.url {
        if !contains_template(url)
            && (url.contains("token=")
            || url.contains("key=")
            || url.contains("secret=")
            || url.contains("password=")
            || url.contains("authorization="))
        {
            *url = mask_secret(url);
        }
    }
}

pub(super) fn parse_dotenv(content: &str) -> HashMap<String, String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let line = line.strip_prefix("export ").unwrap_or(line);
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, raw) = line.split_once('=')?;
            let key = key.trim();
            if !valid_name(key) {
                return None;
            }
            let value = raw.trim();
            let value = if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                &value[1..value.len() - 1]
            } else {
                value.split_once(" #").map_or(value, |(value, _)| value).trim()
            };
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

pub(super) fn interpolate(value: &str, dotenv: &HashMap<String, String>) -> String {
    let process = |name: &str| std::env::var(name);
    interpolate_with(value, dotenv, &process)
}

fn interpolate_with(
    value: &str,
    dotenv: &HashMap<String, String>,
    process: &dyn Fn(&str) -> Result<String, std::env::VarError>,
) -> String {
    let bytes = value.as_bytes();
    let mut output = String::with_capacity(value.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'\\' && bytes.get(index + 1) == Some(&b'$') {
            output.push('$');
            index += 2;
            continue;
        }
        if bytes[index] != b'$' {
            let character = match value[index..].chars().next() {
                Some(c) => c,
                None => return output,
            };
            output.push(character);
            index += character.len_utf8();
            continue;
        }

        let start = index;
        let (name, fallback, end) = if bytes.get(index + 1) == Some(&b'{') {
            let Some(relative_end) = value[index + 2..].find('}') else {
                output.push('$');
                index += 1;
                continue;
            };
            let end = index + 2 + relative_end;
            let expression = &value[index + 2..end];
            let (name, fallback) = expression
                .split_once(":-")
                .map_or((expression, None), |(name, fallback)| {
                    (name, Some(fallback))
                });
            (name, fallback, end + 1)
        } else {
            let mut end = index + 1;
            while bytes.get(end).is_some_and(|byte| {
                byte.is_ascii_alphanumeric() || *byte == b'_'
            }) {
                end += 1;
            }
            if end == index + 1 {
                output.push('$');
                index += 1;
                continue;
            }
            (&value[index + 1..end], None, end)
        };

        if !valid_name(name) {
            output.push_str(&value[start..end]);
        } else if let Ok(resolved) = process(name) {
            output.push_str(&resolved);
        } else if let Some(resolved) = dotenv.get(name) {
            output.push_str(resolved);
        } else if let Some(fallback) = fallback {
            output.push_str(fallback);
        } else {
            output.push_str(&value[start..end]);
        }
        index = end;
    }

    output
}

fn valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
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
        let process = |_: &str| -> Result<String, std::env::VarError> {
            Err(std::env::VarError::NotPresent)
        };
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
    fn secret_masking_handles_empty_short_long_and_template_values() {
        assert_eq!(mask_secret(""), "");
        assert_eq!(mask_secret("short"), "********");
        assert_eq!(mask_secret("sk-123456789"), "****6789");
        assert_eq!(mask_value("API_TOKEN", "raw"), "********");
        assert_eq!(mask_value("API_TOKEN", "${TOKEN}"), "${TOKEN}");
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
        assert_eq!(projected.url.as_deref(), Some("https://mcp.example.test/mcp"));
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
        assert_eq!(rendered["mcpServers"]["test"]["args"][0], "--token=resolved");
        assert_eq!(
            rendered["mcpServers"]["remote"]["url"],
            "https://mcp.example.test/mcp"
        );
        assert_eq!(rendered["mcpServers"]["test"]["headers"]["Authorization"], "Bearer resolved");

        let serialized: Value = serde_json::to_value(&server).unwrap();
        assert_eq!(serialized["env"]["TOKEN"], original);
        assert_eq!(serialized["url"], "https://${MCP_TEST_HOST}/mcp");
        assert_eq!(
            serialized["headers"]["Authorization"],
            "Bearer ${ORQUESTRA_MCP_TEST_SECRET_837491}"
        );
    }
}
