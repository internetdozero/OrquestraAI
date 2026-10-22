//! Master catalog storage: seeding, CRUD, and enabled-state mutation.
//!
//! The catalog lives at `~/.orquestra/mcps.json` and always carries API
//! tokens, so it is sealed with AES-256-GCM (see `crate::at_rest`) and
//! written `0600`. Neither this module nor any other owns
//! the harness configs: mutations here always hand off to [`super::deploy`].

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::deploy::{deploy_current, write_config};
use super::secrets::mask_server;
use super::McpServer;
use crate::harness::{self, GLOBAL_SCOPE};
use crate::official::is_official_mcp;
use crate::paths::OrquestraPaths;

mod mutate;
pub use mutate::{delete_mcp, save_mcp};

/// Optional one-time dataset used to seed the catalog on first launch.
///
/// User-scoped by design: `<root>/seed-mcps.json`, overridable with
/// `ORQUESTRA_SEED_MCP_CATALOG` for operators carrying an external dataset.
/// A world-writable default (the old `/mnt/amnesic` path) would let any
/// local user plant malicious MCP servers into someone else's first
/// launch; whatever path resolves here must additionally pass the symlink
/// refusal in [`seed_catalog`].
pub(super) fn seed_catalog_path(root: &Path) -> PathBuf {
    std::env::var_os("ORQUESTRA_SEED_MCP_CATALOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("seed-mcps.json"))
}

/// Master catalog file name inside `~/.orquestra`.
const CATALOG_FILE: &str = "mcps.json";

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
    url: Option<String>,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    enabled: bool,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Every server in the master catalog, seed included on first launch. The
/// official flag is stamped here so catalogs written before the tier
/// existed still report correctly.
pub fn list_mcps() -> Result<Vec<McpServer>, String> {
    let paths = OrquestraPaths::resolve()?;
    let catalog = load_catalog(&paths, &seed_catalog_path(&paths.root))?;
    Ok(catalog
        .mcps
        .into_values()
        .map(|mut server| {
            server.official = is_official_mcp(&server.id);
            server
        })
        .collect())
}

/// Catalog values intended for the UI: plaintext secrets never cross IPC.
pub fn list_mcps_masked() -> Result<Vec<McpServer>, String> {
    let mut servers = list_mcps()?;
    for server in &mut servers {
        mask_server(server);
    }
    Ok(servers)
}

/// Return a single secret only after an explicit operator reveal action.
pub fn reveal_mcp_secret(id: &str, key: &str) -> Result<String, String> {
    let paths = OrquestraPaths::resolve()?;
    let catalog = load_catalog(&paths, &seed_catalog_path(&paths.root))?;
    let server = catalog
        .mcps
        .get(id)
        .ok_or_else(|| format!("unknown MCP server: {id}"))?;
    if key == "url" {
        return server
            .url
            .clone()
            .ok_or_else(|| format!("unknown MCP secret: {id}.{key}"));
    }
    if let Some(index) = key
        .strip_prefix("arg:")
        .and_then(|value| value.parse::<usize>().ok())
    {
        return server
            .args
            .get(index)
            .cloned()
            .ok_or_else(|| format!("unknown MCP secret: {id}.{key}"));
    }
    server
        .env
        .get(key)
        .or_else(|| server.headers.get(key))
        .cloned()
        .ok_or_else(|| format!("unknown MCP secret: {id}.{key}"))
}

/// Project or un-project one server. `None` (or `global`) edits the baseline
/// that every adapted harness inherits; a harness id edits that harness alone.
pub fn set_mcp_state(id: &str, harness_id: Option<&str>, enabled: bool) -> Result<(), String> {
    let paths = OrquestraPaths::resolve()?;
    let mut catalog = load_catalog(&paths, &seed_catalog_path(&paths.root))?;
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

/// Enable or disable every server for one scope in a single atomic write.
/// `None` (or `global`) flips `global_enabled` on all servers; a harness id
/// adds/removes that harness from every server's `enabled_harnesses` without
/// touching `global_enabled` (so Global inheritance stays intact).
pub fn batch_set_mcp_all(harness_id: Option<&str>, enabled: bool) -> Result<(), String> {
    let paths = OrquestraPaths::resolve()?;
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

    save_catalog(&paths, &catalog)?;
    deploy_current(&paths)
}

/// Reconcile the Global baseline to exactly `enabled` servers. Per-harness
/// opt-ins are operator territory: a preset sets `global_enabled` but never
/// clears `enabled_harnesses`, so a harness that opted into a server keeps
/// it even when the preset drops it globally. Returns how many servers
/// changed global state. The caller deploys afterwards, once.
pub fn apply_global_selection(enabled: &[String]) -> Result<usize, String> {
    let paths = OrquestraPaths::resolve()?;
    apply_global_selection_at(&paths, enabled)
}

/// `apply_global_selection` against explicit paths, for tests and callers
/// that already resolved the storage locations.
pub fn apply_global_selection_at(
    paths: &OrquestraPaths,
    enabled: &[String],
) -> Result<usize, String> {
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
    let paths = OrquestraPaths::resolve()?;
    apply_harness_selections_at(&paths, harness_mcps)
}

/// `apply_harness_selections` against explicit paths, for tests and callers
/// that already resolved the storage locations.
pub fn apply_harness_selections_at(
    paths: &OrquestraPaths,
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
        save_catalog(paths, &catalog)?;
    }

    Ok(changed)
}

pub(super) fn catalog_path(paths: &OrquestraPaths) -> PathBuf {
    paths.root.join(CATALOG_FILE)
}

/// Read the master catalog, seeding it from the audited dataset on first run.
pub(super) fn load_catalog(paths: &OrquestraPaths, seed: &Path) -> Result<Catalog, String> {
    let path = catalog_path(paths);
    refuse_catalog_symlink(&path, "read")?;
    let content = match fs::read_to_string(&path) {
        Ok(content) if !content.trim().is_empty() => content,
        Ok(_) => return Ok(Catalog::default()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let catalog = seed_catalog(seed)?;
            save_catalog(paths, &catalog)?;
            return Ok(catalog);
        }
        Err(error) => {
            return Err(format!("failed to read {}: {error}", path.display()));
        }
    };
    // The at-rest layer owns legacy migration, v1 → v2 upgrades, keyring
    // promotion, and backup self-healing — the loader just parses.
    let (plaintext, _) =
        crate::at_rest::open_stored(paths, &path, &content, crate::at_rest::AAD_MCPS)
            .map_err(|error| format!("failed to unseal {}: {error}", path.display()))?;
    let catalog: Catalog = serde_json::from_str(&plaintext)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    Ok(catalog)
}

/// Populate a first-run catalog from the user-scoped seed dataset. A missing
/// seed file is not an error: the operator can still add servers by hand.
/// A symlinked seed fails closed — unlike the catalog itself, the seed is
/// allowed to be operator-supplied, so a link could redirect first-launch
/// seeding at an attacker-chosen file.
pub(super) fn seed_catalog(seed: &Path) -> Result<Catalog, String> {
    refuse_catalog_symlink(seed, "seed from")?;
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
                official: is_official_mcp(&id),
                id,
                name: entry.name,
                description: entry.description,
                command: entry.command,
                args: entry.args,
                env: entry.env,
                url: entry.url,
                headers: entry.headers,
                global_enabled: entry.enabled,
                enabled_harnesses: Vec::new(),
            },
        );
    }

    Ok(catalog)
}

pub(super) fn save_catalog(paths: &OrquestraPaths, catalog: &Catalog) -> Result<(), String> {
    let path = catalog_path(paths);
    refuse_catalog_symlink(&path, "replace")?;
    let mut content = serde_json::to_string_pretty(catalog)
        .map_err(|error| format!("failed to serialize the MCP catalog: {error}"))?;
    content.push('\n');

    // The catalog carries API tokens, so it is sealed and always private.
    let sealed = crate::at_rest::seal_text(paths, &content, crate::at_rest::AAD_MCPS)?;
    write_config(&path, &sealed, true).map(|_| ())
}

fn refuse_catalog_symlink(path: &Path, verb: &str) -> Result<(), String> {
    if fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(format!(
            "refusing to {verb} {}: not a regular file",
            path.display()
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Deployment
// ---------------------------------------------------------------------------

pub(super) fn validate_server(server: &McpServer) -> Result<(), String> {
    let invalid = server.id.is_empty()
        || server.id == "."
        || server.id == ".."
        || server.id.starts_with('.')
        || server.id.chars().any(|character| {
            character.is_whitespace() || matches!(character, '/' | '\\' | '"' | '\'' | '\0')
        });

    if invalid {
        return Err(format!("invalid MCP id: {:?}", server.id));
    }

    if server.name.trim().is_empty() {
        return Err("an MCP server needs a name".to_string());
    }

    if server.url.is_none() && server.command.trim().is_empty() {
        return Err("an MCP server needs a command".to_string());
    }

    Ok(())
}
