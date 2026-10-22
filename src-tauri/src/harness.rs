//! Harness registry, re-exported from `orquestra-core`.
//!
//! The table and its lookups moved to `crates/orquestra-core/src/harness`
//! (task #326). The glob re-export keeps every `crate::harness::…` import and
//! the crate's public `harness` API unchanged.

pub use orquestra_core::harness::*;

// The config adapter layer (one MCP dialect per harness) lives in
// `orquestra-harness` (task #327); it is re-exported here so callers keep
// using `crate::harness::…` and stay independent of the crate layout.
pub use orquestra_harness::{render_mcp_config, HarnessServer};
