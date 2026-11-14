//! Master catalog storage: seeding, CRUD, and enabled-state mutation.
//!
//! The catalog lives at `~/.ownhub/mcps.json` and always carries API
//! tokens, so it is sealed with AES-256-GCM (see `crate::at_rest`) and
//! written `0600`. Neither this module nor any other owns
//! the harness configs: mutations here always hand off to [`super::deploy`].

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::deploy::{deploy_current, write_config};
use super::secrets::mask_server;
use super::McpServer;
use crate::harness::{self, GLOBAL_SCOPE};
use crate::official::is_official_mcp;
use crate::paths::OwnHubPaths;

mod gemini_import;
mod install;
mod mutate;
mod seed;
mod selection;
pub use mutate::{delete_mcp, save_mcp};
use seed::SeedEntry;
#[cfg(test)]
pub use selection::apply_global_selection_at;
pub use selection::{apply_global_selection, apply_harness_selections, batch_set_mcp_all};

/// Optional one-time dataset used to seed the catalog on first launch.
///
/// User-scoped by design: `<root>/seed-mcps.json`, overridable with
/// `OWNHUB_SEED_MCP_CATALOG` for operators carrying an external dataset.
/// A world-writable default (the old `/mnt/amnesic` path) would let any
/// local user plant malicious MCP servers into someone else's first
/// launch; whatever path resolves here must additionally pass the symlink
/// refusal in [`seed_catalog`].
pub(super) fn seed_catalog_path(root: &Path) -> PathBuf {
    std::env::var_os("OWNHUB_SEED_MCP_CATALOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("seed-mcps.json"))
}

/// Master catalog file name inside `~/.ownhub`.
const CATALOG_FILE: &str = "mcps.json";

/// The catalog plus the bookkeeping needed to clean up after deletions.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Catalog {
    #[serde(default)]
    pub(super) mcps: BTreeMap<String, McpServer>,
    /// Ids OwnHub has written into harness configs, so a server that is
    /// disabled or deleted is removed from those files instead of lingering
    /// there forever. Foreign entries are never touched because they are never
    /// in here.
    #[serde(default)]
    pub(super) managed_ids: BTreeSet<String>,
}

pub(super) fn ensure_active_official(
    catalog: &mut Catalog,
    paths: &OwnHubPaths,
) -> Result<(), String> {
    for server in catalog
        .mcps
        .values_mut()
        .filter(|server| server.active_anywhere())
    {
        install::ensure_official_server(server, paths)?;
    }
    Ok(())
}

/// Every server in the master catalog, seed included on first launch. The
/// official flag is stamped here so catalogs written before the tier
/// existed still report correctly.
pub fn list_mcps() -> Result<Vec<McpServer>, String> {
    let paths = OwnHubPaths::resolve()?;
    let home = crate::bridge::home_dir()?;
    let seed = seed_catalog_path(&paths.root);
    gemini_import::adopt_existing(&paths, &home, &seed)?;
    super::deploy::deploy_current(&paths)?;
    let catalog = load_catalog(&paths, &seed)?;
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
    let paths = OwnHubPaths::resolve()?;
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
    let paths = OwnHubPaths::resolve()?;
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

    if enabled {
        let server = catalog
            .mcps
            .get_mut(id)
            .ok_or_else(|| format!("unknown MCP server: {id}"))?;
        install::ensure_official_server(server, &paths)?;
    }
    save_catalog(&paths, &catalog)?;
    deploy_current(&paths)
}

pub(super) fn catalog_path(paths: &OwnHubPaths) -> PathBuf {
    paths.root.join(CATALOG_FILE)
}

/// Read the master catalog, seeding it from the audited dataset on first run.
pub(super) fn load_catalog(paths: &OwnHubPaths, seed: &Path) -> Result<Catalog, String> {
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
    let mut catalog: Catalog = serde_json::from_str(&plaintext)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    migrate_legacy_storage_paths(&mut catalog);
    Ok(catalog)
}

/// Pre-OwnHub catalogs point their commands at the old storage root
/// (`~/.orquestra/...`), which no longer exists after the rename — every
/// spawn then dies instantly and clients report a closed stdio. Rebase
/// any path segment named `.orquestra` onto `.ownhub` in commands, args
/// and env values at load time; the next save persists the fix. Ids are
/// left alone: harness configs may already reference them.
fn migrate_legacy_storage_paths(catalog: &mut Catalog) {
    for server in catalog.mcps.values_mut() {
        server.command = rebase_storage_path(&server.command);
        for arg in &mut server.args {
            *arg = rebase_storage_path(arg);
        }
        for value in server.env.values_mut() {
            *value = rebase_storage_path(value);
        }
    }
}

fn rebase_storage_path(value: &str) -> String {
    let rebased = value.replace("/.orquestra/", "/.ownhub/");
    rebased
        .strip_suffix("/.orquestra")
        .map(|stem| format!("{stem}/.ownhub"))
        .unwrap_or(rebased)
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

pub(super) fn save_catalog(paths: &OwnHubPaths, catalog: &Catalog) -> Result<(), String> {
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn server(command: &str) -> McpServer {
        McpServer {
            id: "demo".to_string(),
            name: "demo".to_string(),
            description: String::new(),
            command: command.to_string(),
            args: Vec::new(),
            env: std::collections::HashMap::new(),
            url: None,
            headers: std::collections::HashMap::new(),
            global_enabled: false,
            enabled_harnesses: Vec::new(),
            official: false,
        }
    }

    #[test]
    fn legacy_storage_paths_are_rebased_onto_ownhub() {
        assert_eq!(
            rebase_storage_path("/home/u/.orquestra/mcp-servers/x/.venv/bin/x"),
            "/home/u/.ownhub/mcp-servers/x/.venv/bin/x"
        );
        assert_eq!(rebase_storage_path("/home/u/.orquestra"), "/home/u/.ownhub");
        // Untouched: new paths, bare names and id-like substrings.
        assert_eq!(
            rebase_storage_path("/home/u/.ownhub/x"),
            "/home/u/.ownhub/x"
        );
        assert_eq!(rebase_storage_path("npx"), "npx");
        assert_eq!(
            rebase_storage_path("/opt/orquestra-tools/bin"),
            "/opt/orquestra-tools/bin"
        );
    }

    #[test]
    fn catalog_migration_rewrites_commands_args_and_env() {
        let mut catalog = Catalog::default();
        let mut entry = server("/home/u/.orquestra/mcp-servers/demo/.venv/bin/demo");
        entry.args = vec![
            "--vault".to_string(),
            "/home/u/.orquestra/vault.json".to_string(),
        ];
        entry
            .env
            .insert("HOME_DIR".to_string(), "/home/u/.orquestra".to_string());
        catalog.mcps.insert(entry.id.clone(), entry);

        migrate_legacy_storage_paths(&mut catalog);

        let migrated = catalog.mcps.get("demo").expect("demo server");
        assert_eq!(
            migrated.command,
            "/home/u/.ownhub/mcp-servers/demo/.venv/bin/demo"
        );
        assert_eq!(migrated.args[1], "/home/u/.ownhub/vault.json");
        assert_eq!(migrated.env["HOME_DIR"], "/home/u/.ownhub");
    }
}
