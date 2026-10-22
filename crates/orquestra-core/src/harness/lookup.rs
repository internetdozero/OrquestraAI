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
    resolve_install_dir_with(spec, home, &|key| std::env::var(key).ok())
}

/// [`resolve_install_dir`] against an injected environment reader.
///
/// The process environment is shared state: reading it directly makes the
/// resolution untestable without mutating it, and mutating it from a test
/// races every other thread. Callers that already know the environment (the
/// tests, and any future caller resolving several harnesses at once) pass it
/// in instead.
pub fn resolve_install_dir_with(
    spec: &HarnessSpec,
    home: &Path,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> PathBuf {
    if let Some(key) = spec.env_override {
        if let Some(value) = lookup(key) {
            if !value.trim().is_empty() {
                return PathBuf::from(value);
            }
        }
    }
    home.join(spec.install_rel)
}

/// Global roots OrquestraAI provisions itself. Every other link waits for its
/// harness to create the parent folder (a harness that never ran is not given
/// an empty tree), but the Codebuff SDK's `~/.agents/skills` is only reachable
/// through the link, so fusion creates its parent instead of skipping it.
const PROVISIONED_ROOTS: &[&str] = &[".agents/skills"];

/// True when fusion may create the parent of `rel` on its own.
pub fn provisions_parent(rel: &str) -> bool {
    PROVISIONED_ROOTS.contains(&rel)
}

/// The harness that claims `rel` as one of its primary skills roots.
///
/// Some paths are read by several harnesses: `~/.agents/skills` is the
/// Codebuff SDK's canonical root (Freebuff) while Codex and Grok also inject
/// it. The registry names exactly one owner, and fusion defers to that
/// harness's projection instead of treating the shared path as a backdoor.
pub fn primary_owner(rel: &str) -> Option<&'static HarnessSpec> {
    HARNESSES
        .iter()
        .find(|harness| harness.skills_links.contains(&rel))
}

/// True when the harness's install directory exists.
pub fn is_installed(spec: &HarnessSpec, home: &Path) -> bool {
    is_installed_with(spec, home, &|key| std::env::var(key).ok())
}

/// [`is_installed`] against an injected environment reader.
pub fn is_installed_with(
    spec: &HarnessSpec,
    home: &Path,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> bool {
    resolve_install_dir_with(spec, home, lookup).exists()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every primary link has exactly one owner, so fusion never sees a path
    /// that one harness links and another empties.
    #[test]
    fn primary_links_have_a_single_owner() {
        for spec in HARNESSES {
            for rel in spec.skills_links {
                assert_eq!(
                    primary_owner(rel).map(|owner| owner.id),
                    Some(spec.id),
                    "{rel} has a competing owner"
                );
            }
        }
    }

    #[test]
    fn the_shared_agent_root_belongs_to_freebuff() {
        assert_eq!(
            primary_owner(".agents/skills").map(|owner| owner.id),
            Some("freebuff")
        );
        assert!(provisions_parent(".agents/skills"));

        // Codex and Grok read the same path, but only as a secondary location:
        // they never claim it as their own projection.
        for id in ["codex", "grok"] {
            let spec = find(id).expect("registered");
            assert!(!spec.skills_links.contains(&".agents/skills"));
            assert!(spec
                .secondary
                .iter()
                .any(|entry| entry.rel == ".agents/skills"));
        }

        // A harness-specific link still waits for the harness to provision it.
        assert!(!provisions_parent(".codex/skills"));
    }
}
