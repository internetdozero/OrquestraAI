//! Apply saved MCP scope selections to the master catalog.

use std::collections::{BTreeSet, HashMap};

use super::super::deploy::deploy_current;
use super::{ensure_active_official, load_catalog, save_catalog, seed_catalog_path};
use crate::harness::{self, GLOBAL_SCOPE};
use crate::paths::OwnHubPaths;

/// Enable or disable every server for one scope in a single atomic write.
/// `None` (or `global`) flips `global_enabled` on all servers; a harness id
/// adds/removes that harness from every server's `enabled_harnesses` without
/// touching `global_enabled` (so Global inheritance stays intact).
pub fn batch_set_mcp_all(harness_id: Option<&str>, enabled: bool) -> Result<(), String> {
    let paths = OwnHubPaths::resolve()?;
    let scope = harness_id.unwrap_or(GLOBAL_SCOPE);

    if scope != GLOBAL_SCOPE && !harness::is_mcp_target(scope) {
        return Err(format!("{scope} has no MCP adapter yet"));
    }

    let mut catalog = load_catalog(&paths, &seed_catalog_path(&paths.root))?;
    for server in catalog.mcps.values_mut() {
        if scope == GLOBAL_SCOPE {
            server.global_enabled = enabled;
        } else {
            server.enabled_harnesses.retain(|harness| harness != scope);
            if enabled {
                server.enabled_harnesses.push(scope.to_string());
            }
        }
    }

    if enabled {
        ensure_active_official(&mut catalog, &paths)?;
    }
    save_catalog(&paths, &catalog)?;
    deploy_current(&paths)
}

/// Reconcile the Global baseline to exactly `enabled` servers. Per-harness
/// opt-ins are operator territory: a preset sets `global_enabled` but never
/// clears `enabled_harnesses`, so a harness that opted into a server keeps
/// it even when the preset drops it globally. Returns how many servers
/// changed global state. The caller deploys afterwards, once.
pub fn apply_global_selection(enabled: &[String]) -> Result<usize, String> {
    let paths = OwnHubPaths::resolve()?;
    apply_global_selection_at(&paths, enabled)
}

/// `apply_global_selection` against explicit paths, for tests and callers
/// that already resolved the storage locations.
pub fn apply_global_selection_at(paths: &OwnHubPaths, enabled: &[String]) -> Result<usize, String> {
    let mut catalog = load_catalog(paths, &seed_catalog_path(&paths.root))?;
    let wanted: BTreeSet<&str> = enabled.iter().map(String::as_str).collect();
    let mut changed = 0usize;

    for server in catalog.mcps.values_mut() {
        let should_enable = wanted.contains(server.id.as_str());
        if server.global_enabled != should_enable {
            changed += 1;
        }
        server.global_enabled = should_enable;
    }

    if changed > 0 {
        ensure_active_official(&mut catalog, paths)?;
        save_catalog(paths, &catalog)?;
    }

    Ok(changed)
}

/// Reconcile per-harness opt-ins to exactly `harness_mcps`
/// (`harness_id -> server ids`). Authoritative per harness: a server kept
/// out of a harness list is removed from that harness. Unknown harness ids
/// are ignored. Returns how many servers changed harness state.
pub fn apply_harness_selections(
    harness_mcps: &HashMap<String, Vec<String>>,
) -> Result<usize, String> {
    let paths = OwnHubPaths::resolve()?;
    apply_harness_selections_at(&paths, harness_mcps)
}

/// `apply_harness_selections` against explicit paths, for tests and callers
/// that already resolved the storage locations.
fn apply_harness_selections_at(
    paths: &OwnHubPaths,
    harness_mcps: &HashMap<String, Vec<String>>,
) -> Result<usize, String> {
    let mut catalog = load_catalog(paths, &seed_catalog_path(&paths.root))?;
    // Transpose to `server_id -> wanted harnesses`, keeping valid targets.
    let mut wanted: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    for (harness, ids) in harness_mcps {
        if !harness::is_mcp_target(harness) {
            continue;
        }
        for id in ids {
            wanted
                .entry(id.as_str())
                .or_default()
                .insert(harness.as_str());
        }
    }

    let mut changed = 0usize;
    for server in catalog.mcps.values_mut() {
        let mut next: Vec<String> = wanted
            .get(server.id.as_str())
            .map(|set| set.iter().map(|harness| harness.to_string()).collect())
            .unwrap_or_default();
        next.sort();
        let mut current = server.enabled_harnesses.clone();
        current.sort();
        if current != next {
            changed += 1;
        }
        server.enabled_harnesses = next;
    }

    if changed > 0 {
        ensure_active_official(&mut catalog, paths)?;
        save_catalog(paths, &catalog)?;
    }

    Ok(changed)
}
