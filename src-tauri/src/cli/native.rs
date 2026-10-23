//! Native subcommands that expose the app's existing MCP-backed capabilities.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

mod bridge;
mod ideas;
mod sessions;
mod vault;

#[derive(Debug, PartialEq, Eq)]
pub enum CliError {
    Usage(String),
    Operation(String),
}

/// Recognized native CLI roots return output/errors; unrelated args fall back
/// to the existing desktop launcher and legacy flags.
pub fn dispatch_native_cli(args: &[String]) -> Option<Result<String, CliError>> {
    match args.first().map(String::as_str) {
        Some("ideas") => Some(ideas::run(&args[1..])),
        Some("sessions") => Some(sessions::run(&args[1..])),
        Some("bridge") => Some(bridge::run(&args[1..])),
        Some("vault") => Some(vault::run(&args[1..])),
        _ => None,
    }
}

#[derive(Default)]
struct Options {
    values: HashMap<String, Vec<String>>,
    switches: HashSet<String>,
}

impl Options {
    fn one(&self, name: &str) -> Result<Option<String>, CliError> {
        match self.values.get(name).map(Vec::as_slice) {
            None => Ok(None),
            Some([value]) => Ok(Some(value.clone())),
            Some(_) => Err(usage(format!("{name} may only be supplied once"))),
        }
    }

    fn required(&self, name: &str) -> Result<String, CliError> {
        self.one(name)?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| usage(format!("missing required {name}")))
    }

    fn has(&self, name: &str) -> bool {
        self.switches.contains(name)
    }

    fn repeated(&self, name: &str) -> Vec<String> {
        self.values.get(name).cloned().unwrap_or_default()
    }
}

fn parse_options(
    args: &[String],
    value_options: &[&str],
    switches: &[&str],
) -> Result<Options, CliError> {
    let mut parsed = Options::default();
    let mut index = 0;
    while index < args.len() {
        let option = args[index].as_str();
        if switches.contains(&option) {
            parsed.switches.insert(option.to_string());
            index += 1;
            continue;
        }
        if !value_options.contains(&option) {
            return Err(usage(format!("unknown option {option}")));
        }
        let value = args
            .get(index + 1)
            .filter(|value| !value.starts_with('-'))
            .ok_or_else(|| usage(format!("{option} requires a value")))?;
        parsed
            .values
            .entry(option.to_string())
            .or_default()
            .push(value.clone());
        index += 2;
    }
    Ok(parsed)
}

fn usage(message: String) -> CliError {
    CliError::Usage(message)
}

fn rendered<T: Serialize>(value: &T, as_json: bool, human: String) -> Result<String, CliError> {
    if as_json {
        serde_json::to_string_pretty(value)
            .map_err(|error| CliError::Operation(format!("JSON serialization failed: {error}")))
    } else {
        Ok(human)
    }
}

fn parse_u32(options: &Options, name: &str) -> Result<Option<u32>, CliError> {
    options
        .one(name)?
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| usage(format!("{name} must be a non-negative integer")))
        })
        .transpose()
}

fn parse_i64(options: &Options, name: &str) -> Result<Option<i64>, CliError> {
    options
        .one(name)?
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| usage(format!("{name} must be an integer")))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_cli_supports_human_and_json_output() {
        let value = serde_json::json!({ "ready": true });
        assert_eq!(
            rendered(&value, false, "ready".to_string()),
            Ok("ready".to_string())
        );
        assert_eq!(
            rendered(&value, true, "ready".to_string()),
            Ok("{\n  \"ready\": true\n}".to_string())
        );
    }

    #[test]
    fn recognized_roots_reject_unknown_subcommands() {
        let args = ["ideas".to_string(), "unknown".to_string()];
        assert!(matches!(
            dispatch_native_cli(&args),
            Some(Err(CliError::Usage(_)))
        ));
    }
}
