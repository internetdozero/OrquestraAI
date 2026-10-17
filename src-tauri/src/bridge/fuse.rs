//! Backup-and-link fusion.
//!
//! Fusion never destroys state: harness folders are archived into a run
//! backup before they are replaced, harness-only skills are adopted into
//! the vault, and hidden bookkeeping (Codex `.system`, Hermes curator
//! files) is carried into the projection.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::status::{
    is_link, is_linked_to, parent_is_dir, path_present, skill_folders,
    statuses_at, PRIMARY_WALK, SECONDARY_WALK, Walk,
};
use super::BridgeReport;
use crate::harness::{self, HarnessSpec, SecondaryAction};
use crate::paths::{copy_dir_all, OrquestraPaths};
use crate::skills;

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
            if !home.join(spec.install_rel).is_dir() {
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

/// Move every entry out of `source` into `destination`, leaving an empty
/// directory behind so a harness that expects the folder still finds it.
fn quarantine_dir(source: &Path, destination: &Path) -> Result<usize, String> {
    let entries = fs::read_dir(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?;

    let mut moved = 0usize;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("failed to read {}: {error}", source.display()))?;
        backup_move(&entry.path(), &destination.join(entry.file_name()))?;
        moved += 1;
    }

    Ok(moved)
}

fn link_instructions(
    source: &Path,
    paths: &OrquestraPaths,
    backup: &Path,
    spec: &HarnessSpec,
    rel: &str,
    imported: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let master = paths.agents_path();

    if is_link(source) {
        if is_linked_to(source, &master) {
            return Ok(());
        }
        backup_move(source, &backup_slot(backup, spec.id, rel))?;
    } else if source.is_file() {
        if let Ok(content) = fs::read_to_string(source) {
            if !content.trim().is_empty() {
                imported.push((spec.label.to_string(), content));
            }
        }
        backup_move(source, &backup_slot(backup, spec.id, rel))?;
    } else if source.exists() {
        backup_move(source, &backup_slot(backup, spec.id, rel))?;
    } else if !source
        .parent()
        .map(|parent| parent.is_dir())
        .unwrap_or(false)
    {
        return Ok(());
    }

    ensure_symlink(source, &master)
}

fn ensure_config(
    config: &Path,
    projection: &Path,
    backup: &Path,
    spec: &HarnessSpec,
    rel: &str,
) -> Result<(), String> {
    let scope = projection.to_string_lossy().into_owned();

    if config.is_file() {
        let content = fs::read_to_string(config)
            .map_err(|error| format!("failed to read {}: {error}", config.display()))?;

        let Some(updated) = merge_skills_path(&content, &scope)? else {
            return Ok(());
        };

        backup_copy(config, &backup_slot(backup, spec.id, rel))?;
        write_atomic(config, &updated)?;
    } else if config
        .parent()
        .map(|parent| parent.is_dir())
        .unwrap_or(false)
    {
        let fresh = format!(
            "{{\n  \"skills\": {{\n    \"paths\": [\"{}\"]\n  }}\n}}\n",
            escape_json(&scope)
        );
        write_atomic(config, &fresh)?;
    }

    Ok(())
}

/// Add `scope` to `skills.paths`, preserving comments and every other key.
/// Returns `None` when the path is already registered.
pub(super) fn merge_skills_path(content: &str, scope: &str) -> Result<Option<String>, String> {
    let value: serde_json::Value = json5::from_str(content)
        .map_err(|error| format!("config is not valid JSONC: {error}"))?;

    let root = value
        .as_object()
        .ok_or_else(|| "config root is not an object".to_string())?;

    let registered = root
        .get("skills")
        .and_then(|skills| skills.get("paths"))
        .and_then(|paths| paths.as_array())
        .map(|paths| paths.iter().any(|entry| entry.as_str() == Some(scope)))
        .unwrap_or(false);

    if registered {
        return Ok(None);
    }

    if !root.contains_key("skills") {
        let closing = content
            .rfind('}')
            .ok_or_else(|| "config has no closing brace".to_string())?;

        // Drop trailing whitespace and an optional trailing comma (JSONC allows
        // both) so the injected key stays syntactically valid.
        let mut head = content[..closing].to_string();
        let trimmed = head.trim_end().len();
        head.truncate(trimmed);
        if head.ends_with(',') {
            head.pop();
            let trimmed = head.trim_end().len();
            head.truncate(trimmed);
        }

        let mut updated = String::with_capacity(content.len() + 96);
        updated.push_str(&head);
        updated.push_str(if root.is_empty() { "\n" } else { ",\n" });
        updated.push_str(&format!(
            "  \"skills\": {{\n    \"paths\": [\"{}\"]\n  }}\n",
            escape_json(scope)
        ));
        updated.push_str(&content[closing..]);

        json5::from_str::<serde_json::Value>(&updated)
            .map_err(|error| format!("refusing to write an invalid config: {error}"))?;

        return Ok(Some(updated));
    }

    // `skills` exists without our path: merge structurally and rewrite.
    let mut merged = value;
    if let Some(skills) = merged
        .get_mut("skills")
        .and_then(|skills| skills.as_object_mut())
    {
        match skills.get_mut("paths") {
            Some(paths) if paths.is_array() => {
                if let Some(array) = paths.as_array_mut() {
                    array.push(serde_json::Value::String(scope.to_string()));
                }
            }
            _ => {
                skills.insert(
                    "paths".to_string(),
                    serde_json::Value::Array(vec![serde_json::Value::String(scope.to_string())]),
                );
            }
        }
    }

    let mut rewritten = serde_json::to_string_pretty(&merged)
        .map_err(|error| format!("failed to serialize config: {error}"))?;
    rewritten.push('\n');
    Ok(Some(rewritten))
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

fn ensure_symlink(link: &Path, target: &Path) -> Result<(), String> {
    if is_linked_to(link, target) {
        return Ok(());
    }

    if let Ok(metadata) = fs::symlink_metadata(link) {
        if metadata.file_type().is_symlink() || metadata.is_file() {
            fs::remove_file(link).map_err(|error| {
                format!("failed to clear {}: {error}", link.display())
            })?;
        } else {
            return Err(format!(
                "refusing to replace the directory {} with a symlink",
                link.display()
            ));
        }
    }

    if let Some(parent) = link.parent() {
        create_dir_all(parent)?;
    }

    std::os::unix::fs::symlink(target, link)
        .map_err(|error| format!("failed to link {} -> {}: {error}", link.display(), target.display()))
}

fn backup_slot(backup: &Path, id: &str, rel: &str) -> PathBuf {
    backup.join(id).join(rel.replace('/', "__"))
}

fn backup_move(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        create_dir_all(parent)?;
    }

    if fs::rename(source, destination).is_ok() {
        return Ok(());
    }

    // Different filesystem: copy then remove.
    move_entry(source, destination)
}

fn move_entry(source: &Path, destination: &Path) -> Result<(), String> {
    if is_link(source) {
        let target = fs::read_link(source)
            .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
        let _ = fs::remove_file(destination);
        std::os::unix::fs::symlink(target, destination).map_err(|error| {
            format!("failed to back up symlink {}: {error}", source.display())
        })?;
        fs::remove_file(source)
            .map_err(|error| format!("failed to remove {}: {error}", source.display()))?;
        return Ok(());
    }

    if source.is_dir() {
        copy_dir_all(source, destination)?;
        fs::remove_dir_all(source)
            .map_err(|error| format!("failed to remove {}: {error}", source.display()))?;
        return Ok(());
    }

    fs::copy(source, destination)
        .map_err(|error| format!("failed to copy {}: {error}", source.display()))?;
    fs::remove_file(source)
        .map_err(|error| format!("failed to remove {}: {error}", source.display()))
}

fn backup_copy(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        create_dir_all(parent)?;
    }

    fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| format!("failed to back up {}: {error}", source.display()))
}

pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    create_dir_all(parent)?;

    let temporary = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "config".to_string()),
        std::process::id()
    ));

    fs::write(&temporary, content).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to write {}: {error}", temporary.display())
    })?;

    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to replace {}: {error}", path.display())
    })
}

pub(crate) fn create_dir_all(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| format!("failed to create {}: {error}", path.display()))
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn timestamp_label() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format_timestamp(seconds)
}

pub(super) fn format_timestamp(unix_seconds: u64) -> String {
    let days = (unix_seconds / 86_400) as i64;
    let remainder = unix_seconds % 86_400;
    let (year, month, day) = civil_from_days(days);

    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        remainder / 3_600,
        (remainder % 3_600) / 60,
        remainder % 60
    )
}

/// Howard Hinnant's days-from-civil algorithm, inverted.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 { shifted } else { shifted - 146_096 } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;

    (if month <= 2 { year + 1 } else { year }, month, day)
}
