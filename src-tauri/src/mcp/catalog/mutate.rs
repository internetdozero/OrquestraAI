//! Insert, replace and delete one MCP server, including masked-secret restore.

use std::collections::HashMap;
use std::path::Path;

use super::super::secrets::{mask_server, mask_value, restore_masked_args};
use super::super::McpServer;
use super::{deploy_current, load_catalog, save_catalog, validate_server, SEED_CATALOG};
use crate::harness;
use crate::official::is_official_mcp;
use crate::paths::OrquestraPaths;

/// Insert or replace one server, then re-compile the harness configs.
pub fn save_mcp(server: McpServer) -> Result<(), String> {
    validate_server(&server)?;

    let paths = OrquestraPaths::resolve()?;
    let mut catalog = load_catalog(&paths, Path::new(SEED_CATALOG))?;

    let mut server = server;
    // Recompute: the flag is derived from the id, never client state.
    server.official = is_official_mcp(&server.id);
    server
        .enabled_harnesses
        .retain(|harness| harness::is_mcp_target(harness));
    server.enabled_harnesses.sort();
    server.enabled_harnesses.dedup();

    if let Some(previous) = catalog.mcps.get(&server.id) {
        let mut masked_previous = previous.clone();
        mask_server(&mut masked_previous);
        restore_masked_values(&mut server.env, &previous.env);
        restore_masked_values(&mut server.headers, &previous.headers);
        restore_masked_args(&mut server.args, &previous.args);
        if server.url == masked_previous.url {
            server.url = previous.url.clone();
        }
    }

    catalog.mcps.insert(server.id.clone(), server);
    save_catalog(&paths, &catalog)?;
    deploy_current(&paths)
}

fn restore_masked_values(
    incoming: &mut HashMap<String, String>,
    previous: &HashMap<String, String>,
) {
    for (key, current) in incoming {
        let Some(original) = previous.get(key) else {
            continue;
        };
        let masked = mask_value(key, original);
        if masked != original.as_str() && *current == masked {
            *current = original.clone();
        }
    }
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
