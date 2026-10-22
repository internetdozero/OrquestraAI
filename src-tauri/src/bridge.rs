//! Harness fusion, re-exported from `orquestra-bridge`.
//!
//! The backup-and-link work, the secondary-location sweep and the status
//! reporting moved to `crates/orquestra-bridge` (task #329), which depends on
//! `orquestra-core` instead of this shell. These re-exports keep every
//! `crate::bridge::…` call site — the tray panel, the MCP deploy, presets, the
//! CLI flags and the IPC commands — unchanged.

pub use orquestra_bridge::{
    deploy_harness_bridge, get_bridge_status, home_dir, BridgeReport, HarnessStatus,
    SecondaryAction, SecondaryStatus,
};

// Backup folder names are shared with the MCP deploy; keep the old crate-only
// visibility so it does not become part of this crate's public API.
pub(crate) use orquestra_bridge::timestamp_label;
