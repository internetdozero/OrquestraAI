//! Granular CLI management for OrquestraAI Sessions (`orquestra sessions …`).

use super::{usage, CliError};

mod delete;
mod export;
mod handoff;
mod inspect;
mod list;
mod pins;
mod policy;
mod purge;
mod search;
#[cfg(test)]
mod tests;
mod watch;

const USAGE: &str = "usage: orquestra sessions <list|inspect|pin|unpin|tail|watch|handoff|search|delete|export|purge|policy> [options]";

pub(super) fn is_session_active(harness: &str, session_id: &str) -> bool {
    let Ok(home) = crate::sessions::adapters::home() else {
        return false;
    };
    crate::sessions::adapters::adapters_by_id(&home)
        .get(harness)
        .is_some_and(|adapter| adapter.is_session_active(session_id))
}

pub(super) fn run(args: &[String]) -> Result<String, CliError> {
    match args.first().map(String::as_str) {
        Some("list") => list::list(&args[1..]),
        Some("inspect" | "info" | "show") => inspect::inspect(&args[1..]),
        Some("pin") => pins::pin(&args[1..]),
        Some("unpin") => pins::unpin(&args[1..]),
        Some("tail") => watch::tail(&args[1..]),
        Some("watch") => watch::watch(&args[1..]),
        Some("handoff") => handoff::handoff(&args[1..]),
        Some("search") => search::search(&args[1..]),
        Some("delete" | "rm") => delete::delete(&args[1..]),
        Some("export") => export::export(&args[1..]),
        Some("purge") => purge::purge(&args[1..]),
        Some("policy") => policy::policy(&args[1..]),
        _ => Err(usage(USAGE.to_string())),
    }
}
