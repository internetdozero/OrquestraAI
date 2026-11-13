//! Backup-and-link fusion.
//!
//! Fusion never destroys state: harness folders are archived into a run
//! backup before they are replaced, harness-only skills are adopted into
//! the vault, and hidden bookkeeping (Codex `.system`, Hermes curator
//! files) is carried into the projection.

use std::fs;
use std::path::{Path, PathBuf};

use super::status::{
    ancestor_is_symlink, is_link, is_linked_to, parent_is_dir, path_present, skill_folders,
    statuses_at, PRIMARY_WALK, SECONDARY_WALK,
};
use super::BridgeReport;
use orquestra_core::agents::{read_agents, write_agents, DEFAULT_AGENTS};
use orquestra_core::harness::{self, HarnessSpec, SecondaryAction};
use orquestra_core::paths::{rebase, OrquestraPaths};
use orquestra_core::skills::SKILL_FILE;

mod adopt;
mod backup;
mod fuse_io;
use adopt::adopt_missing_skills;
use backup::{finalize_backup, next_backup_path, retain_recent_backups};
pub use fuse_io::timestamp_label;
use fuse_io::{
    backup_move, backup_slot, create_dir_all, ensure_config, ensure_symlink, link_instructions,
    move_entry, quarantine_dir,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(super) use fuse_io::{format_timestamp, merge_skills_path};

pub(super) fn deploy_at(home: &Path, paths: &OrquestraPaths) -> Result<BridgeReport, String> {
    paths.ensure_layout()?;

    // Capture the master instructions before anything is replaced so a pristine
    // file can absorb the harness instruction files instead of dropping them.
    let master_before = read_agents(paths)?;

    let before = statuses_at(home, paths);
    let needs_work = before
        .iter()
        .any(|status| status.supported && status.installed && !status.bridged);

    let mut adopted = Vec::new();
    let mut quarantined = Vec::new();
    let mut imported: Vec<(String, String)> = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut backup_dir: Option<PathBuf> = None;

    if needs_work {
        let backup = next_backup_path(paths);
        backup_dir = Some(backup.clone());

        for spec in harness::HARNESSES {
            if !harness::is_installed(spec, home) {
                continue;
            }

            if let Err(error) = fuse_harness(
                home,
                paths,
                &backup,
                spec,
                &mut adopted,
                &mut quarantined,
                &mut imported,
                &mut warnings,
            ) {
                errors.push(format!("{}: {error}", spec.id));
            }
        }

        if master_before == DEFAULT_AGENTS {
            // The import publishes the master through a rename, which detaches
            // any hard link made above: remember those links first and re-point
            // them at the file the import wrote.
            let file_links = fuse_io::file_linked_instructions(home, paths);
            if let Err(error) = import_instructions(paths, &imported) {
                errors.push(format!("AGENTS.md import: {error}"));
            }
            errors.extend(fuse_io::resync_file_links(&file_links, paths));
        }

        // The run may only need to create missing links. Allocate no backup
        // directory unless an operation actually preserved something.
        backup_dir = backup_dir.map(finalize_backup).transpose()?.flatten();
    }

    let harnesses = statuses_at(home, paths);
    let bridged = harnesses.iter().filter(|status| status.bridged).count();
    // Only prune after a complete deployment. On a partial failure, keep every
    // older recovery point so the operator can still roll back safely.
    let installed_bridged = harnesses
        .iter()
        .filter(|status| status.supported && status.installed)
        .all(|status| status.bridged);
    if backup_dir.is_some() && errors.is_empty() && installed_bridged {
        if let Err(error) = retain_recent_backups(&paths.root.join("backups")) {
            warnings.push(format!("backup retention: {error}"));
        }
    }

    Ok(BridgeReport {
        backup_dir: backup_dir.map(|path| path.to_string_lossy().into_owned()),
        bridged,
        total: harnesses.len(),
        harnesses,
        adopted,
        quarantined,
        instructions_imported: imported.iter().map(|(label, _)| label.clone()).collect(),
        warnings,
        errors,
    })
}

// Keep the caller's per-run accumulators explicit; bundling them would obscure
// which outcomes this harness contributes to the final bridge report.
#[allow(clippy::too_many_arguments)]
fn fuse_harness(
    home: &Path,
    paths: &OrquestraPaths,
    backup: &Path,
    spec: &HarnessSpec,
    adopted: &mut Vec<String>,
    quarantined: &mut Vec<String>,
    imported: &mut Vec<(String, String)>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let projection = paths.projection_dir(spec.id);
    create_dir_all(&projection)?;

    for rel in spec.skills_links {
        let source = rebase(home, rel);
        if ancestor_is_symlink(home, &source) {
            continue;
        }
        link_skills(
            &source,
            &projection,
            &paths.vault,
            backup,
            spec,
            rel,
            adopted,
            warnings,
        )?;
    }

    for rel in spec.instructions_rels {
        let source = rebase(home, rel);
        if !ancestor_is_symlink(home, &source) {
            link_instructions(&source, paths, backup, spec, rel, imported)?;
        }
    }

    if let Some(rel) = spec.config_rel {
        let source = rebase(home, rel);
        if !ancestor_is_symlink(home, &source) {
            ensure_config(&source, &projection, backup, spec, rel)?;
        }
    }

    fuse_secondary(home, paths, backup, spec, adopted, quarantined, warnings)?;

    Ok(())
}

// These are the complete inputs needed to preserve and project one skills path.
#[allow(clippy::too_many_arguments)]
fn link_skills(
    source: &Path,
    projection: &Path,
    vault: &Path,
    backup: &Path,
    spec: &HarnessSpec,
    rel: &str,
    adopted: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if is_link(source) {
        if is_linked_to(source, projection) {
            return Ok(());
        }
        backup_move(source, &backup_slot(backup, spec.id, rel))?;
    } else if source.is_dir() {
        adopt_missing_skills(source, vault, adopted, PRIMARY_WALK)?;
        import_visible_skills(source, vault, projection)?;
        preserve_hidden_entries(source, projection, warnings)?;
        backup_move(source, &backup_slot(backup, spec.id, rel))?;
    } else if source.exists() {
        backup_move(source, &backup_slot(backup, spec.id, rel))?;
    } else if !source
        .parent()
        .map(|parent| parent.is_dir())
        .unwrap_or(false)
    {
        // Harness root is not provisioned for this path; nothing to bridge.
        return Ok(());
    }

    ensure_symlink(source, projection)
}

/// Keep the harness's current skill set visible after the swap: everything it
/// already had becomes a projection link back into the vault, so fusion never
/// silently empties a harness the operator was using.
fn import_visible_skills(source: &Path, vault: &Path, projection: &Path) -> Result<(), String> {
    for (name, _) in skill_folders(source, PRIMARY_WALK)? {
        let vault_item = vault.join(&name);
        if !vault_item.join(SKILL_FILE).is_file() {
            continue;
        }

        let link = projection.join(&name);
        if fs::symlink_metadata(&link).is_ok() {
            continue;
        }

        orquestra_core::links::create_dir_link(&vault_item, &link).map_err(|error| {
            format!(
                "failed to project {} into {}: {error}",
                name,
                projection.display()
            )
        })?;
    }

    Ok(())
}

/// Move hidden state (Codex `.system`, Hermes curator files) into the projection
/// so the harness keeps its own bookkeeping after the symlink swap.
fn preserve_hidden_entries(
    source: &Path,
    projection: &Path,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with('.') {
            continue;
        }

        let destination = projection.join(&name);
        if destination.exists() {
            warnings.push(format!(
                "kept existing {} instead of the one from the harness",
                destination.display()
            ));
            continue;
        }

        move_entry(&entry.path(), &destination)?;
    }

    Ok(())
}

/// Claim the locations a harness reads beside its projection. A harness that
/// discovers vendor checkouts, plugin marketplaces, or bundled repositories
/// stays bloated no matter what the operator toggles off, because those paths
/// never pass through `active/<id>`.
fn fuse_secondary(
    home: &Path,
    paths: &OrquestraPaths,
    backup: &Path,
    spec: &HarnessSpec,
    adopted: &mut Vec<String>,
    quarantined: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let projection = paths.projection_dir(spec.id);

    for entry in spec.secondary {
        let source = rebase(home, entry.rel);
        let slot = backup_slot(backup, spec.id, entry.rel);
        if ancestor_is_symlink(home, &source) {
            continue;
        }

        // A path another harness projects as its own skills root belongs to
        // that harness: emptying it here would break the owner's link.
        if harness::primary_owner(entry.rel)
            .is_some_and(|owner| owner.id != spec.id && harness::is_installed(owner, home))
        {
            continue;
        }

        if !path_present(&source) {
            // Nothing to archive. A projectable folder still gains the link
            // when its parent exists, so a harness update that recreates the
            // folder lands in the projection instead of a fresh backdoor.
            if entry.action == SecondaryAction::Project && parent_is_dir(&source) {
                ensure_symlink(&source, &projection)?;
            }
            continue;
        }

        match entry.action {
            SecondaryAction::Project => {
                if is_linked_to(&source, &projection) {
                    continue;
                }

                if source.is_dir() && !is_link(&source) {
                    adopt_missing_skills(&source, &paths.vault, adopted, SECONDARY_WALK)?;
                }

                backup_move(&source, &slot)?;
                ensure_symlink(&source, &projection)?;
                quarantined.push(entry.rel.to_string());
            }
            SecondaryAction::Quarantine => {
                if is_link(&source) {
                    backup_move(&source, &slot)?;
                    quarantined.push(entry.rel.to_string());
                    continue;
                }

                if source.is_dir() {
                    adopt_missing_skills(&source, &paths.vault, adopted, SECONDARY_WALK)?;

                    let moved = quarantine_dir(&source, &slot)?;
                    if moved > 0 {
                        quarantined.push(entry.rel.to_string());
                        warnings.push(format!(
                            "{}: moved {moved} {} out of {} into the run backup",
                            spec.label,
                            if moved == 1 { "entry" } else { "entries" },
                            entry.rel
                        ));
                    }
                    continue;
                }

                backup_move(&source, &slot)?;
                quarantined.push(entry.rel.to_string());
            }
            SecondaryAction::AdoptOnly => {
                if source.is_dir() && !is_link(&source) {
                    let before = adopted.len();
                    adopt_missing_skills(&source, &paths.vault, adopted, SECONDARY_WALK)?;

                    if adopted.len() > before {
                        warnings.push(format!(
                            "{}: {} ships its own built-ins — they are archived in the \
                             vault but still load from the harness",
                            spec.label, entry.rel
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

fn import_instructions(
    paths: &OrquestraPaths,
    imported: &[(String, String)],
) -> Result<(), String> {
    let mut merged = DEFAULT_AGENTS.to_string();
    let mut seen: Vec<String> = Vec::new();

    for (label, content) in imported {
        let trimmed = content.trim();
        if trimmed.is_empty() || seen.iter().any(|previous| previous == trimmed) {
            continue;
        }

        merged.push_str(&format!("\n---\n\n## Imported from {label}\n\n{trimmed}\n"));
        seen.push(trimmed.to_string());
    }

    if merged == DEFAULT_AGENTS {
        return Ok(());
    }

    write_agents(paths, &merged)
}
