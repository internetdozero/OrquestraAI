//! Harness registry, re-exported from `orquestra-core`.
//!
//! The table and its lookups moved to `crates/orquestra-core/src/harness`
//! (task #326). The glob re-export keeps every `crate::harness::…` import and
//! the crate's public `harness` API unchanged.

pub use orquestra_core::harness::*;
