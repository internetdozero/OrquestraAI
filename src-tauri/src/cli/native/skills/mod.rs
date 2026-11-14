//! Granular CLI management for Skills (`ownhub skills …`).

use super::{usage, CliError};

mod inspect;
mod list;
mod manage;
mod mutate;
#[cfg(test)]
mod tests;

const USAGE: &str = "usage: ownhub skills <list|info|inspect|enable|disable|batch|summary|validate|create|scaffold|import|export|content|delete> [options]";

pub(super) fn run(args: &[String]) -> Result<String, CliError> {
    match args.first().map(String::as_str) {
        Some("list") => list::list(&args[1..]),
        Some("summary") => list::summary(&args[1..]),
        Some("info" | "inspect") => inspect::inspect(&args[1..]),
        Some("validate") => inspect::validate(&args[1..]),
        Some("enable") => mutate::enable(&args[1..]),
        Some("disable") => mutate::disable(&args[1..]),
        Some("batch") => mutate::batch(&args[1..]),
        Some("delete") => mutate::delete(&args[1..]),
        Some("create" | "scaffold") => manage::create(&args[1..]),
        Some("import") => manage::import(&args[1..]),
        Some("export") => manage::export(&args[1..]),
        Some("content") => manage::content(&args[1..]),
        _ => Err(usage(USAGE.to_string())),
    }
}
