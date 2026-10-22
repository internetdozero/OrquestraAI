//! `orquestra-core`: the foundational layer every OrquestraAI crate shares.
//!
//! Phase 1 of the modularisation (task #326) lifts the storage layout, the
//! harness registry and the common error type out of the desktop shell so
//! that later crates can depend on them without depending on Tauri.
//!
//! The shell re-exports these items from its own `paths` and `harness`
//! modules, so existing `crate::paths::…` and `crate::harness::…` imports —
//! and the crate's public API — are unchanged.

pub mod agents;
pub mod error;
pub mod harness;
pub mod paths;
pub mod skills;

pub use agents::{read_agents, write_agents, DEFAULT_AGENTS};
pub use error::{CoreError, CoreResult};
pub use harness::{find, harness_ids, HarnessSpec, GLOBAL_SCOPE, HARNESSES};
pub use paths::OrquestraPaths;
