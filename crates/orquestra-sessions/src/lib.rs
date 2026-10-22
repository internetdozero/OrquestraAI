//! `orquestra-sessions`: the read-only catalog of every conversation the
//! harnesses keep on disk.
//!
//! Two layers live here, both free of the Tauri runtime:
//!
//! - [`types`] is the wire contract with the Sessions tab: one
//!   [`types::UnifiedSession`] per conversation, the transcript turns, and
//!   the pin/retention/purge payloads the commands exchange.
//! - [`adapters`] is the [`adapters::SessionAdapter`] contract plus one
//!   implementation per harness (Claude, Codex, Freebuff, …), the shared
//!   filesystem/transcript/SQLite helpers they build on, and the registry
//!   that merges an installed fleet into one listing.
//!
//! Listing never writes to a harness store: a harness that is broken, locked
//! or absent is reported and skipped, so one bad database cannot empty the
//! tab. Mutating operations (pins, retention, purge) and the commands that
//! expose them stay in the desktop shell, which calls into this crate.

pub mod adapters;
pub mod types;
