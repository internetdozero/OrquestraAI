//! Backup-and-link fusion.
//!
//! Fusion never destroys state: harness folders are archived into a run
//! backup before they are replaced, harness-only skills are adopted into
//! the vault, and hidden bookkeeping (Codex `.system`, Hermes curator
//! files) is carried into the projection.

use std::fs;
use std::path::{Path, PathBuf};

use super::status::{
    is_link, is_linked_to, parent_is_dir, path_present, skill_folders,
    statuses_at, PRIMARY_WALK, SECONDARY_WALK, Walk,
};
use super::BridgeReport;
use crate::harness::{self, HarnessSpec, SecondaryAction};
use crate::paths::{copy_dir_all, OrquestraPaths};
use crate::skills;

mod fuse_io;
use fuse_io::{
    backup_move, backup_slot, create_dir_all, ensure_config, ensure_symlink, link_instructions,
    move_entry, quarantine_dir,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(super) use fuse_io::{format_timestamp, merge_skills_path};
pub(crate) use fuse_io::timestamp_label;

const BACKUP_PREFIX: &str = "pre-fusion";

pub(super) fn deploy_at(home: &Path, paths: &OrquestraPaths) -> Result<BridgeReport, String> {
    paths.ensure_layout()?;

    // Capture the master instructions before anything is replaced so a pristine
    // file can absorb the harness instruction files instead of dropping them.
    let master_before = skills::read_agents(paths)?;

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
        let backup = paths
            .root
            .join("backups")
            .join(format!("{BACKUP_PREFIX}-{}", timestamp_label()));
        create_dir_all(&backup)?;
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

        if master_before == skills::DEFAULT_AGENTS {
            if let Err(error) = import_instructions(paths, &imported) {
                errors.push(format!("AGENTS.md import: {error}"));
            }
        }
    }

    let harnesses = statuses_at(home, paths);
    let bridged = harnesses.iter().filter(|status| status.bridged).count();

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
        let source = home.join(rel);
        link_skills(&source, &projection, &paths.vault, backup, spec, rel, adopted, warnings)?;
    }

    if let Some(rel) = spec.instructions_rel {
        link_instructions(&home.join(rel), paths, backup, spec, rel, imported)?;
    }

    if let Some(rel) = spec.config_rel {
        ensure_config(&home.join(rel), &projection, backup, spec, rel)?;
    }

    fuse_secondary(home, paths, backup, spec, adopted, quarantined, warnings)?;

    Ok(())
}

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

/// Copy skill folders that exist only inside a harness into the master vault so
/// nothing becomes invisible after the directory is replaced by a symlink.
fn adopt_missing_skills(
    source: &Path,
    vault: &Path,
    adopted: &mut Vec<String>,
    walk: Walk,
) -> Result<(), String> {
    for (name, path) in skill_folders(source, walk)? {
        let destination = vault.join(&name);
        if destination.exists() {
            continue;
        }

        copy_dir_all(&path, &destination)?;
        adopted.push(name);
    }

    Ok(())
}

/// Keep the harness's current skill set visible after the swap: everything it
/// already had becomes a projection link back into the vault, so fusion never
/// silently empties a harness the operator was using.
fn import_visible_skills(source: &Path, vault: &Path, projection: &Path) -> Result<(), String> {
    for (name, _) in skill_folders(source, PRIMARY_WALK)? {
        let vault_item = vault.join(&name);
        if !vault_item.join(skills::SKILL_FILE).is_file() {
            continue;
        }

        let link = projection.join(&name);
        if fs::symlink_metadata(&link).is_ok() {
            continue;
        }

        std::os::unix::fs::symlink(&vault_item, &link).map_err(|error| {
            format!("failed to project {} into {}: {error}", name, projection.display())
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
        let source = home.join(entry.rel);
        let slot = backup_slot(backup, spec.id, entry.rel);

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
                    adopt_missing_skills(
                        &source,
                        &paths.vault,
                        adopted,
                        SECONDARY_WALK,
                    )?;
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
                    adopt_missing_skills(
                        &source,
                        &paths.vault,
                        adopted,
                        SECONDARY_WALK,
                    )?;

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
                    adopt_missing_skills(
                        &source,
                        &paths.vault,
                        adopted,
                        SECONDARY_WALK,
                    )?;

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


fn import_instructions(paths: &OrquestraPaths, imported: &[(String, String)]) -> Result<(), String> {
    let mut merged = skills::DEFAULT_AGENTS.to_string();
    let mut seen: Vec<String> = Vec::new();

    for (label, content) in imported {
        let trimmed = content.trim();
        if trimmed.is_empty() || seen.iter().any(|previous| previous == trimmed) {
            continue;
        }

        merged.push_str(&format!("\n---\n\n## Imported from {label}\n\n{trimmed}\n"));
        seen.push(trimmed.to_string());
    }

    if merged == skills::DEFAULT_AGENTS {
        return Ok(());
    }

    skills::write_agents(paths, &merged)
}

