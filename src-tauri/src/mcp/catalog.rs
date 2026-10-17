//! Master catalog storage: seeding, CRUD, and enabled-state mutation.
//!
//! The catalog lives at `~/.orquestra/mcps.json` and always carries API
//! tokens, so it is written `0600`. Neither this module nor any other owns
//! the harness configs: mutations here always hand off to [`super::deploy`].

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::deploy::{deploy_current, write_config};
use super::McpServer;
use crate::harness::{self, GLOBAL_SCOPE};
use crate::paths::OrquestraPaths;

/// Master catalog file name inside `~/.orquestra`.
const CATALOG_FILE: &str = "mcps.json";

/// One-time dataset used to seed a fresh catalog on first launch.
pub(super) const SEED_CATALOG: &str = "/mnt/amnesic/temporary_mcps/MASTER_MCP_CATALOG.json";

/// The catalog plus the bookkeeping needed to clean up after deletions.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Catalog {
    #[serde(default)]
    pub(super) mcps: BTreeMap<String, McpServer>,
    /// Ids OrquestraAI has written into harness configs, so a server that is
    /// disabled or deleted is removed from those files instead of lingering
    /// there forever. Foreign entries are never touched because they are never
    /// in here.
    #[serde(default)]
    pub(super) managed_ids: BTreeSet<String>,
}

/// Shape of one entry in the audited machine dataset.
#[derive(Debug, Deserialize)]
pub(super) struct SeedEntry {
    #[serde(default)]
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    enabled: bool,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Every server in the master catalog, seed included on first launch.
pub fn list_mcps() -> Result<Vec<McpServer>, String> {
    let paths = OrquestraPaths::resolve()?;
    let catalog = load_catalog(&paths, Path::new(SEED_CATALOG))?;
    Ok(catalog.mcps.into_values().collect())
}

/// Project or un-project one server. `None` (or `global`) edits the baseline
/// that every adapted harness inherits; a harness id edits that harness alone.
pub fn set_mcp_state(id: &str, harness_id: Option<&str>, enabled: bool) -> Result<(), String> {
    let paths = OrquestraPaths::resolve()?;
    let mut catalog = load_catalog(&paths, Path::new(SEED_CATALOG))?;
    let scope = harness_id.unwrap_or(GLOBAL_SCOPE);

    {
        let server = catalog
            .mcps
            .get_mut(id)
            .ok_or_else(|| format!("unknown MCP server: {id}"))?;

        if scope == GLOBAL_SCOPE {
            server.global_enabled = enabled;
        } else {
            if !harness::is_mcp_target(scope) {
                return Err(format!("{scope} has no MCP adapter yet"));
            }
            if server.global_enabled && !enabled {
                return Err(format!(
                    "“{id}” is inherited from Global; disable it in Global first"
                ));
            }

            server.enabled_harnesses.retain(|harness| harness != scope);
            if enabled {
                server.enabled_harnesses.push(scope.to_string());
            }
        }
    }

    save_catalog(&paths, &catalog)?;
    deploy_current(&paths)
}

/// Reconcile the whole catalog to exactly `enabled` global servers, clearing
/// the per-harness opt-ins so every harness sees the same profile. Returns how
/// many servers changed state. The caller deploys afterwards, once.
pub fn apply_global_selection(enabled: &[String]) -> Result<usize, String> {
    let paths = OrquestraPaths::resolve()?;
    let mut catalog = load_catalog(&paths, Path::new(SEED_CATALOG))?;
    let wanted: BTreeSet<&str> = enabled.iter().map(String::as_str).collect();
    let mut changed = 0usize;

    for server in catalog.mcps.values_mut() {
        let should_enable = wanted.contains(server.id.as_str());
        if server.global_enabled != should_enable || !server.enabled_harnesses.is_empty() {
            changed += 1;
        }
        server.global_enabled = should_enable;
        server.enabled_harnesses.clear();
    }

    if changed > 0 {
        save_catalog(&paths, &catalog)?;
    }

    Ok(changed)
}

/// Insert or replace one server, then re-compile the harness configs.
pub fn save_mcp(server: McpServer) -> Result<(), String> {
    validate_server(&server)?;

    let paths = OrquestraPaths::resolve()?;
    let mut catalog = load_catalog(&paths, Path::new(SEED_CATALOG))?;

    let mut server = server;
    server.enabled_harnesses.retain(|harness| harness::is_mcp_target(harness));
    server.enabled_harnesses.sort();
    server.enabled_harnesses.dedup();

    catalog.mcps.insert(server.id.clone(), server);
    save_catalog(&paths, &catalog)?;
    deploy_current(&paths)
}

/// Drop a server from the catalog. It stays in `managedIds` long enough for the
/// deploy to strip it out of every harness config it was written into.
pub fn delete_mcp(id: &str) -> Result<(), String> {
    let paths = OrquestraPaths::resolve()?;
    let mut catalog = load_catalog(&paths, Path::new(SEED_CATALOG))?;

    if catalog.mcps.remove(id).is_none() {
        return Err(format!("unknown MCP server: {id}"));
    }

    save_catalog(&paths, &catalog)?;
    deploy_current(&paths)
}

fn catalog_path(paths: &OrquestraPaths) -> PathBuf {
    paths.root.join(CATALOG_FILE)
}

/// Read the master catalog, seeding it from the audited dataset on first run.
pub(super) fn load_catalog(paths: &OrquestraPaths, seed: &Path) -> Result<Catalog, String> {
    let path = catalog_path(paths);
    match fs::read_to_string(&path) {
        Ok(content) if !content.trim().is_empty() => serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse {}: {error}", path.display())),
        Ok(_) => Ok(Catalog::default()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let catalog = seed_catalog(seed)?;
            save_catalog(paths, &catalog)?;
            Ok(catalog)
        }
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}

/// Populate a first-run catalog from the audited machine dataset. A missing
/// seed file is not an error: the operator can still add servers by hand.
pub(super) fn seed_catalog(seed: &Path) -> Result<Catalog, String> {
    if !seed.is_file() {
        return Ok(Catalog::default());
    }

    let content = fs::read_to_string(seed)
        .map_err(|error| format!("failed to read {}: {error}", seed.display()))?;
    let entries: BTreeMap<String, SeedEntry> = serde_json::from_str(&content)
        .map_err(|error| format!("failed to parse {}: {error}", seed.display()))?;

    let mut catalog = Catalog::default();
    for (key, entry) in entries {
        let id = if entry.id.trim().is_empty() {
            key.clone()
        } else {
            entry.id.clone()
        };

        catalog.mcps.insert(
            id.clone(),
            McpServer {
                id,
                name: entry.name,
                description: entry.description,
                command: entry.command,
                args: entry.args,
                env: entry.env,
                global_enabled: entry.enabled,
                enabled_harnesses: Vec::new(),
            },
        );
    }

    Ok(catalog)
}

pub(super) fn save_catalog(paths: &OrquestraPaths, catalog: &Catalog) -> Result<(), String> {
    let mut content = serde_json::to_string_pretty(catalog)
        .map_err(|error| format!("failed to serialize the MCP catalog: {error}"))?;
    content.push('\n');

    // The catalog carries API tokens, so it is always private.
    write_config(&catalog_path(paths), &content, true).map(|_| ())
}

// ---------------------------------------------------------------------------
// Deployment
// ---------------------------------------------------------------------------

pub(super) fn validate_server(server: &McpServer) -> Result<(), String> {
    let invalid = server.id.is_empty()
        || server.id == "."
        || server.id == ".."
        || server.id.starts_with('.')
        || server
            .id
            .chars()
            .any(|character| character.is_whitespace() || matches!(character, '/' | '\\' | '"' | '\'' | '\0'));

    if invalid {
        return Err(format!("invalid MCP id: {:?}", server.id));
    }

    if server.name.trim().is_empty() {
        return Err("an MCP server needs a name".to_string());
    }

    if server.command.trim().is_empty() {
        return Err("an MCP server needs a command".to_string());
    }

    Ok(())
}
