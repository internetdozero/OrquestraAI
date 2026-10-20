use std::collections::HashMap;

pub(super) fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "token",
        "key",
        "secret",
        "authorization",
        "password",
        "bearer",
    ]
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
        bytes
            .get(index + 1)
            .is_some_and(|byte| matches!(*byte, b'{' | b'_' | b'A'..=b'Z' | b'a'..=b'z'))
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

/// Restore a masked arg wherever it sits. Index-only matching stores the
/// `****` placeholder after a flag is inserted in front of the secret.
pub(super) fn restore_masked_args(incoming: &mut [String], previous: &[String]) {
    let mut probe = super::McpServer {
        id: String::new(),
        name: String::new(),
        description: String::new(),
        command: String::new(),
        args: previous.to_vec(),
        env: HashMap::new(),
        url: None,
        headers: HashMap::new(),
        global_enabled: false,
        enabled_harnesses: Vec::new(),
        official: false,
    };
    mask_server(&mut probe);
    let masked = probe.args;

    for slot in incoming.iter_mut() {
        let mut hits = masked
            .iter()
            .enumerate()
            .filter(|(_, value)| *value == slot);
        let Some((index, _)) = hits.next() else {
            continue;
        };
        if hits.next().is_some() {
            continue;
        }
        if masked[index] != previous[index] {
            *slot = previous[index].clone();
        }
    }
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
        // Query keys arrive in any case (`API_KEY=`). Match the folded URL
        // so a capitalised secret does not cross IPC in plaintext.
        let folded = url.to_ascii_lowercase();
        if !contains_template(url)
            && ["token=", "key=", "secret=", "password=", "authorization="]
                .iter()
                .any(|marker| folded.contains(marker))
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
            Some((key.to_string(), dotenv_value(raw)))
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
            while bytes
                .get(end)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            {
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

/// Quoted values keep an internal `#`. A comment after the closing quote used
/// to leave the quotes in the value, so `TOKEN='secret' # note` became
/// `'secret'` and the deployed server received the wrong credential.
fn dotenv_value(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(quote) = raw
        .chars()
        .next()
        .filter(|quote| *quote == '"' || *quote == '\'')
    {
        if let Some(end) = raw[1..].find(quote) {
            let rest = raw[1 + end + 1..].trim_start();
            if rest.is_empty() || rest.starts_with('#') {
                return raw[1..1 + end].to_string();
            }
        }
    }
    raw.split_once(" #")
        .map(|(value, _)| value.trim_end())
        .unwrap_or(raw)
        .to_string()
}

fn valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[cfg(test)]
#[path = "secrets_tests.rs"]
mod tests;
