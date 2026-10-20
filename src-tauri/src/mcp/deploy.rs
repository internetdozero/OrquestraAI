//! Compiling the catalog into every installed harness config.
//!
//! Each target is read fresh, re-rendered in its own dialect, and only then
//! replaced — atomically, never loosening a file's mode, and with the
//! previous bytes kept in `~/.orquestra/backups/pre-mcp-*`.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use super::catalog::{load_catalog, save_catalog, SEED_CATALOG};
use super::secrets::parse_dotenv;
use super::serializers::{render_codex_toml, render_jsonc_block, render_mcp_servers};
use super::{McpServer, McpTargetStatus};
use crate::bridge;
use crate::harness::{self, McpFormat};
use crate::paths::OrquestraPaths;

const BACKUP_PREFIX: &str = "pre-mcp";

/// Harness configs OrquestraAI can compile MCP servers into.
pub fn list_targets() -> Result<Vec<McpTargetStatus>, String> {
    let home = bridge::home_dir()?;
    harness::mcp_targets()
        .map(|spec| -> Result<McpTargetStatus, String> {
            let mcp = spec
                .mcp
                .as_ref()
                .ok_or_else(|| format!("harness {:?} has no MCP spec; bug", spec.id))?;
            Ok(McpTargetStatus {
                id: spec.id.to_string(),
                label: spec.label.to_string(),
                config_path: home.join(mcp.rel).to_string_lossy().into_owned(),
                installed: harness::is_installed(spec, &home),
            })
        })
        .collect::<Result<Vec<_>, _>>()
}

/// Compile the catalog into every installed harness's own format.
pub fn deploy_mcps() -> Result<(), String> {
    deploy_mcps_reporting().map(|_| ())
}

/// Same deploy, but reports which config files changed and which had to be
/// tightened to `0600` because they now carry env values.
pub fn deploy_mcps_reporting() -> Result<Vec<String>, String> {
    let paths = OrquestraPaths::resolve()?;
    paths.ensure_layout()?;
    let home = bridge::home_dir()?;
    deploy_at(&home, &paths, Path::new(SEED_CATALOG))
}

pub(super) fn deploy_current(paths: &OrquestraPaths) -> Result<(), String> {
    let home = bridge::home_dir()?;
    deploy_at(&home, paths, Path::new(SEED_CATALOG)).map(|_| ())
}

// ---------------------------------------------------------------------------
// Catalog storage
// ---------------------------------------------------------------------------

pub(super) fn deploy_at(
    home: &Path,
    paths: &OrquestraPaths,
    seed: &Path,
) -> Result<Vec<String>, String> {
    let mut catalog = load_catalog(paths, seed)?;
    let dotenv_path = paths.root.join(".env");
    if fs::symlink_metadata(&dotenv_path)
        .ok()
        .is_some_and(|meta| meta.file_type().is_symlink())
    {
        return Err(format!(
            "refusing to read {}: not a regular file",
            dotenv_path.display()
        ));
    }
    let dotenv = match fs::read_to_string(&dotenv_path) {
        Ok(content) => parse_dotenv(&content),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Default::default(),
        Err(error) => return Err(format!("failed to read {}: {error}", dotenv_path.display())),
    };
    let mut changed = Vec::new();
    let mut failed = Vec::new();
    let mut backup_dir: Option<PathBuf> = None;

    for spec in harness::mcp_targets() {
        if !harness::is_installed(spec, home) {
            continue;
        }

        let Some(mcp) = spec.mcp.as_ref() else {
            continue;
        };

        let path = home.join(mcp.rel);
        // `read_to_string` follows, including a real file reached through a
        // directory symlink. Either case would merge the outside config and
        // write it back through the link.
        if ancestor_is_symlink(home, &path)
            || fs::symlink_metadata(&path)
                .ok()
                .is_some_and(|meta| meta.file_type().is_symlink())
        {
            continue;
        }
        let existing = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => {
                failed.push(format!("{}: {error}", spec.label));
                continue;
            }
        };

        let active: Vec<McpServer> = catalog
            .mcps
            .values()
            .filter(|server| server.covers(spec.id))
            .map(|server| server.resolved(&dotenv))
            .collect();
        let stale: BTreeSet<String> = catalog
            .managed_ids
            .iter()
            .filter(|id| !active.iter().any(|server| &server.id == *id))
            .cloned()
            .collect();

        // Nothing wanted and nothing of ours to clean up: leave the file be.
        if active.is_empty() && stale.is_empty() {
            continue;
        }

        let rendered = match mcp.format {
            McpFormat::JsonMcpServers => {
                render_mcp_servers(&existing, &active, &stale, mcp.entry_type)
            }
            McpFormat::JsoncMcpBlock => render_jsonc_block(&existing, &active, &stale),
            McpFormat::CodexToml => render_codex_toml(&existing, &active, &stale),
        };

        let rendered = match rendered {
            Ok(rendered) => rendered,
            Err(error) => {
                failed.push(format!("{}: {error}", spec.label));
                continue;
            }
        };

        // Already in sync: no write, no backup, no timestamp churn.
        if rendered.trim() == existing.trim() {
            continue;
        }

        let secret = active.iter().any(|server| {
            !server.env.is_empty() || !server.headers.is_empty() || server.url.is_some()
        });

        let result = if existing.trim().is_empty() {
            Ok(())
        } else {
            // One backup folder per deploy run, for every file it touches.
            if backup_dir.is_none() {
                backup_dir = Some(backup_run_dir(paths)?);
            }

            backup_config(
                &path,
                backup_dir.as_deref().unwrap_or(paths.root.as_path()),
                spec.id,
                mcp.rel,
                &existing,
            )
        }
        .and_then(|()| write_config(&path, &rendered, secret));

        match result {
            Ok(tightened) => {
                let mut label = path.to_string_lossy().into_owned();
                if tightened {
                    label.push_str(" (mode tightened to 0600)");
                }
                changed.push(label);
            }
            Err(error) => failed.push(format!("{}: {error}", spec.label)),
        }
    }

    // Remember what OrquestraAI owns so a later run can remove it again. Only
    // shrink the set when every target agreed; otherwise keep the union so the
    // removals are retried once the failure is fixed.
    let wanted: BTreeSet<String> = catalog
        .mcps
        .values()
        .filter(|server| server.active_anywhere())
        .map(|server| server.id.clone())
        .collect();
    catalog.managed_ids = if failed.is_empty() {
        wanted
    } else {
        catalog.managed_ids.union(&wanted).cloned().collect()
    };
    save_catalog(paths, &catalog)?;

    if failed.is_empty() {
        Ok(changed)
    } else {
        Err(failed.join("; "))
    }
}

/// Backup folder for one deploy run. Timestamps only have second resolution, so
/// two runs in the same second must not share (and overwrite) a folder.
fn backup_run_dir(paths: &OrquestraPaths) -> Result<PathBuf, String> {
    let stamp = bridge::timestamp_label();
    let backups = paths.root.join("backups");

    fs::create_dir_all(&backups)
        .map_err(|error| format!("failed to create {}: {error}", backups.display()))?;

    for attempt in 0..1_000 {
        let candidate = if attempt == 0 {
            backups.join(format!("{BACKUP_PREFIX}-{stamp}"))
        } else {
            backups.join(format!("{BACKUP_PREFIX}-{stamp}-{attempt}"))
        };

        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("failed to create {}: {error}", candidate.display())),
        }
    }

    Err("could not allocate a backup folder".to_string())
}

/// True when a directory between `root` and `path` is a symlink. The last
/// component is ignored so a symlinked file can be reported separately.
fn ancestor_is_symlink(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return true;
    };
    let mut current = root.to_path_buf();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        if components.peek().is_none() {
            break;
        }
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

fn backup_config(
    source: &Path,
    backup: &Path,
    id: &str,
    rel: &str,
    content: &str,
) -> Result<(), String> {
    let destination = backup.join(id).join(format!(
        "{}.bak",
        Path::new(rel)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| id.to_string())
    ));

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    fs::write(&destination, content)
        .map_err(|error| format!("failed to back up {}: {error}", destination.display()))?;

    // The backup holds tokens too: match the original file's privacy.
    let mode = fs::metadata(source)
        .map(|meta| meta.permissions().mode() & 0o777)
        .unwrap_or(0o600);
    let _ = fs::set_permissions(&destination, fs::Permissions::from_mode(mode));

    Ok(())
}

/// Atomic write that never loosens an existing file's mode. Files that will
/// carry env values are kept at `0600`, because harness configs hold API tokens
/// and two of the JSONC targets ship world-readable.
pub(super) fn write_config(path: &Path, content: &str, secret: bool) -> Result<bool, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;

    let mut mode = fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o777)
        .unwrap_or(0o600);
    let mut tightened = false;
    if secret && mode & 0o077 != 0 {
        mode = 0o600;
        tightened = true;
    }

    let temporary = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "config".to_string()),
        std::process::id()
    ));

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(&temporary)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;

    let write = file
        .write_all(content.as_bytes())
        .and_then(|()| file.flush());
    drop(file);

    if let Err(error) = write {
        let _ = fs::remove_file(&temporary);
        return Err(format!("failed to write {}: {error}", temporary.display()));
    }

    fs::set_permissions(&temporary, fs::Permissions::from_mode(mode)).map_err(|error| {
        format!(
            "failed to set permissions on {}: {error}",
            temporary.display()
        )
    })?;

    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to replace {}: {error}", path.display())
    })?;

    Ok(tightened)
}

// ---------------------------------------------------------------------------
// Adapters
// ---------------------------------------------------------------------------
