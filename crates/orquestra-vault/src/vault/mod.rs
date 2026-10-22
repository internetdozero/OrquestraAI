//! Blind Credential Vault & MCP blind proxy.
//!
//! Agents invoke HTTP APIs and CLI commands with stored secrets WITHOUT the
//! raw secret ever entering their context: the backend injects the active key
//! in memory (`proxy` / `exec`) and returns scrubbed output. `store` exposes
//! masked metadata only.
//!
//! [`service`] holds the operations the desktop shell's IPC commands wrap:
//! list, save, delete, primary-key selection, key removal, exec and the
//! migration-backup purge.

mod atomic;
mod endpoints;
pub mod exec;
mod exec_policy;
pub mod http;
pub mod presets;
pub mod proxy;
mod scrub;
pub mod service;
mod store;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub use exec::{child_env, scrub_output};
pub use http::{effective_headers, scrub_body, scrub_text, strip_response_headers};
pub use presets::PresetSpec;
pub use proxy::{request, test_connection, VaultHttpResult, VaultTestResult};
pub use service::{SaveServiceInput, VaultExecResult};
pub use store::{mask_key, KeyHealth, VaultServicePublic};
