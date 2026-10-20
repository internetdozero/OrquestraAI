//! Lookups over the harness registry.

use std::path::{Path, PathBuf};

use super::{HarnessSpec, HARNESSES};

/// Every harness id, in registry order.
pub fn harness_ids() -> impl Iterator<Item = &'static str> {
    HARNESSES.iter().map(|harness| harness.id)
}

/// Look up one harness by id.
pub fn find(id: &str) -> Option<&'static HarnessSpec> {
    HARNESSES.iter().find(|harness| harness.id == id)
}

/// Where a harness is installed. A set, non-empty `env_override` wins;
/// otherwise the standard home-relative default applies.
pub fn resolve_install_dir(spec: &HarnessSpec, home: &Path) -> PathBuf {
    if let Some(key) = spec.env_override {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                return PathBuf::from(value);
            }
        }
    }
    home.join(spec.install_rel)
}

/// True when the harness's install directory exists.
pub fn is_installed(spec: &HarnessSpec, home: &Path) -> bool {
    resolve_install_dir(spec, home).exists()
}

/// True when the harness has an MCP adapter, so a scope can be targeted.
pub fn is_mcp_target(id: &str) -> bool {
    HARNESSES
        .iter()
        .any(|harness| harness.id == id && harness.mcp.is_some())
}

/// Every harness the MCP serializer can write, in registry order.
pub fn mcp_targets() -> impl Iterator<Item = &'static HarnessSpec> {
    HARNESSES.iter().filter(|harness| harness.mcp.is_some())
}
