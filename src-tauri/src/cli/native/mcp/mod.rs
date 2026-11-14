//! Granular CLI management for Model Context Protocol (MCP) servers (`ownhub mcp …`).

use super::{usage, CliError};

mod deploy;
mod inspect;
mod list;
mod mutate;
mod protocol;
#[cfg(test)]
mod tests;

const USAGE: &str = "usage: ownhub mcp <list|status|inspect|add|remove|enable|disable|set-state|deploy|targets|test-connection|tools-list|call-tool> [options]";

pub(super) fn run(args: &[String]) -> Result<String, CliError> {
    match args.first().map(String::as_str) {
        Some("list") => list::list(&args[1..]),
        Some("status") => list::status(&args[1..]),
        Some("inspect" | "info") => inspect::inspect(&args[1..]),
        Some("add") => mutate::add(&args[1..]),
        Some("remove" | "delete") => mutate::remove(&args[1..]),
        Some("enable") => mutate::enable(&args[1..]),
        Some("disable") => mutate::disable(&args[1..]),
        Some("set-state") => mutate::set_state(&args[1..]),
        Some("deploy") => deploy::deploy(&args[1..]),
        Some("targets") => deploy::targets(&args[1..]),
        Some("test-connection" | "test" | "check") => protocol::test_connection(&args[1..]),
        Some("tools-list" | "tools") => protocol::tools_list(&args[1..]),
        Some("call-tool" | "call") => protocol::call_tool(&args[1..]),
        _ => Err(usage(USAGE.to_string())),
    }
}
