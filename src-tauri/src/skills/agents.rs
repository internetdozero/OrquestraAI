//! The master `AGENTS.md`, re-exported from `ownhub-core`.
//!
//! The template, the create-on-first-use read and the atomic write moved to
//! `crates/ownhub-core/src/agents.rs` (task #329): harness fusion in
//! `ownhub-bridge` links every harness to this file, so it cannot live in
//! the shell any more.

pub use ownhub_core::agents::{read_master_agents_md, write_master_agents_md};

// Only the projection tests exercise the template file directly; the shell
// itself reads it through the two functions above.
#[cfg(test)]
pub(crate) use ownhub_core::agents::{read_agents, write_agents};
