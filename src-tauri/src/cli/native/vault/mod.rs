//! Granular CLI management for Blind Credential Vault (`ownhub vault …`).

use super::{usage, CliError};
use crate::paths::OwnHubPaths;

mod audit;
mod mutate;
mod proxy;
mod query;
#[cfg(test)]
mod tests;

const USAGE: &str = "usage: ownhub vault <list|inspect|set|add-key|remove-key|remove|enable|disable|toggle|set-enabled|set-primary|rotate|test|proxy-http|proxy-exec|presets|audit> [options]";

pub(super) fn run(args: &[String]) -> Result<String, CliError> {
    match args.first().map(String::as_str) {
        Some("list") => query::list(&args[1..]),
        Some("inspect" | "show" | "info") => query::inspect(&args[1..]),
        Some("presets") => query::presets(&args[1..]),
        Some("set" | "add" | "save") => mutate::set(&args[1..]),
        Some("add-key") => mutate::add_key(&args[1..]),
        Some("remove-key") => mutate::remove_key(&args[1..]),
        Some("remove" | "delete") => mutate::remove(&args[1..]),
        Some("enable") => mutate::enable(&args[1..]),
        Some("disable") => mutate::disable(&args[1..]),
        Some("toggle") => mutate::toggle(&args[1..]),
        Some("set-enabled") => mutate::set_enabled(&args[1..]),
        Some("set-primary") => mutate::set_primary(&args[1..]),
        Some("rotate") => mutate::rotate(&args[1..]),
        Some("test" | "check" | "ping") => proxy::test_conn(&args[1..]),
        Some("proxy-http" | "http") => proxy::http(&args[1..]),
        Some("proxy-exec" | "exec") => proxy::exec(&args[1..]),
        Some("audit") => audit::audit(&args[1..]),
        _ => Err(usage(USAGE.to_string())),
    }
}

pub(super) fn paths() -> Result<OwnHubPaths, CliError> {
    OwnHubPaths::resolve().map_err(|error| CliError::Operation(error.to_string()))
}

pub(super) fn parse_key_number(value: &str) -> Result<usize, CliError> {
    let number = value
        .parse::<usize>()
        .map_err(|_| usage("--key-number must be a positive integer".to_string()))?;
    if number == 0 {
        return Err(usage("--key-number must be at least 1".to_string()));
    }
    Ok(number)
}
