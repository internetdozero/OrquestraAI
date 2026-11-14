//! Granular CLI management for Idea Vault and Blackboard Notes (`ownhub idea …`).

use super::{usage, CliError};

mod idea_ops;
mod note_ops;
mod render;
#[cfg(test)]
mod tests;

const USAGE: &str = "usage: ownhub idea <list|get|add|update|status|delete|search|projects|export-md|metrics|note-post|note-list|note-get|note-read|note-clear> [options]";

pub(super) fn run(args: &[String]) -> Result<String, CliError> {
    match args.first().map(String::as_str) {
        Some("help" | "--help" | "-h") => Err(usage(USAGE.to_string())),
        Some("list") => idea_ops::list(&args[1..]),
        Some("get" | "inspect") => idea_ops::get(&args[1..]),
        Some("add" | "record") => idea_ops::add(&args[1..]),
        Some("update") => idea_ops::update(&args[1..]),
        Some("status") => idea_ops::status(&args[1..]),
        Some("delete") => idea_ops::delete(&args[1..]),
        Some("search") => idea_ops::search(&args[1..]),
        Some("projects") => idea_ops::projects(&args[1..]),
        Some("export-md") => idea_ops::export_md(&args[1..]),
        Some("metrics") => idea_ops::metrics(&args[1..]),
        Some("note-post") => note_ops::note_post(&args[1..]),
        Some("note-list") => note_ops::note_list(&args[1..]),
        Some("note-get") => note_ops::note_get(&args[1..]),
        Some("note-read") => note_ops::note_read(&args[1..]),
        Some("note-clear") => note_ops::note_clear(&args[1..]),
        _ => Err(usage(USAGE.to_string())),
    }
}
