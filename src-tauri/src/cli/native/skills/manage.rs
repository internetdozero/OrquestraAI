//! Skill management: create, scaffold, import, export, and content.

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use super::super::{parse_options, rendered, usage, CliError};
use crate::paths::{copy_dir_all, OrquestraPaths};
use crate::skills::{
    import_skill, import_skill_from_git, read_skill_content, save_skill_content, set_skill_state,
};

pub(super) fn create(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra skills create <id> [--name <str>] [--description <str>] [--category <cat>] [--with-scripts] [--enable] [--json]".to_string()))?;

    let options = parse_options(
        &args[1..],
        &["--name", "--description", "--category"],
        &["--with-scripts", "--enable", "--json"],
    )?;

    let name = options.one("--name")?.unwrap_or_else(|| id.replace('-', " "));
    let desc = options.one("--description")?.unwrap_or_else(|| format!("Description for {name}."));
    let cat = options.one("--category")?.unwrap_or_else(|| "general".to_string());
    let with_scripts = options.has("--with-scripts");
    let enable_after = options.has("--enable");

    let paths = OrquestraPaths::resolve().map_err(|e| CliError::Operation(e.to_string()))?;
    let skill_dir = paths.vault.join(id);
    if skill_dir.exists() {
        return Err(CliError::Operation(format!("skill '{id}' already exists in vault")));
    }

    fs::create_dir_all(&skill_dir)
        .map_err(|e| CliError::Operation(format!("failed to create skill directory: {e}")))?;

    let skill_md_content = format!(
        "---\nname: \"{}\"\ndescription: \"{}\"\ncategory: \"{}\"\n---\n\n# {}\n\n{}\n",
        name, desc, cat, name, desc
    );
    fs::write(skill_dir.join("SKILL.md"), skill_md_content)
        .map_err(|e| CliError::Operation(format!("failed to write SKILL.md: {e}")))?;

    if with_scripts {
        let scripts_dir = skill_dir.join("scripts");
        fs::create_dir_all(&scripts_dir)
            .map_err(|e| CliError::Operation(format!("failed to create scripts directory: {e}")))?;
        fs::write(scripts_dir.join("run.sh"), "#!/usr/bin/env bash\necho \"Running skill helper script\"\n")
            .map_err(|e| CliError::Operation(format!("failed to write sample script: {e}")))?;
    }

    if enable_after {
        let _ = set_skill_state(id, "global", true);
    }

    let report = serde_json::json!({
        "id": id,
        "name": name,
        "description": desc,
        "category": cat,
        "hasScripts": with_scripts,
        "enabled": enable_after,
        "path": skill_dir.display().to_string(),
    });
    let human = format!("Created new skill '{id}' in vault ({}).", skill_dir.display());
    rendered(&report, options.has("--json"), human)
}

pub(super) fn import(args: &[String]) -> Result<String, CliError> {
    let source = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra skills import <path|url> [--subpath <path>] [--scope <scope>] [--enable] [--json]".to_string()))?;

    let options = parse_options(
        &args[1..],
        &["--subpath", "--scope"],
        &["--enable", "--json"],
    )?;

    let scope = options.one("--scope")?.unwrap_or_else(|| "global".to_string());
    let enable = options.has("--enable");

    let is_git = source.starts_with("http://") || source.starts_with("https://") || source.starts_with("git@");

    let skill = if is_git {
        let mut final_url = source.clone();
        if let Some(sub) = options.one("--subpath")? {
            if !final_url.contains('#') {
                final_url = format!("{final_url}#{sub}");
            }
        }
        import_skill_from_git(final_url).map_err(CliError::Operation)?
    } else {
        import_skill(source.clone()).map_err(CliError::Operation)?
    };

    if enable {
        let _ = set_skill_state(&skill.id, &scope, true);
    }

    let human = format!("Imported skill '{}' into vault.", skill.id);
    rendered(&skill, options.has("--json"), human)
}

pub(super) fn export(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra skills export <id> [--out <dir>] [--tar] [--json]".to_string()))?;

    let options = parse_options(&args[1..], &["--out"], &["--tar", "--json"])?;
    let out_dir = options.one("--out")?.unwrap_or_else(|| ".".to_string());
    let tar = options.has("--tar");

    let paths = OrquestraPaths::resolve().map_err(|e| CliError::Operation(e.to_string()))?;
    let src_dir = paths.vault.join(id);
    if !src_dir.is_dir() {
        return Err(CliError::Operation(format!("skill '{id}' not found in vault")));
    }

    let target_path = PathBuf::from(&out_dir);
    if tar {
        let archive_file = target_path.join(format!("{id}.tar.gz"));
        let status = std::process::Command::new("tar")
            .args(["-czf", archive_file.to_string_lossy().as_ref(), "-C", paths.vault.to_string_lossy().as_ref(), id])
            .status()
            .map_err(|e| CliError::Operation(format!("tar execution failed: {e}")))?;
        if !status.success() {
            return Err(CliError::Operation("failed to create tar archive".to_string()));
        }
        let report = serde_json::json!({ "id": id, "archive": archive_file.display().to_string() });
        let human = format!("Exported skill '{id}' as archive to '{}'.", archive_file.display());
        return rendered(&report, options.has("--json"), human);
    }

    let dest = target_path.join(id);
    copy_dir_all(&src_dir, &dest).map_err(|e| CliError::Operation(e.to_string()))?;

    let report = serde_json::json!({ "id": id, "destination": dest.display().to_string() });
    let human = format!("Exported skill '{id}' to '{}'.", dest.display());
    rendered(&report, options.has("--json"), human)
}

pub(super) fn content(args: &[String]) -> Result<String, CliError> {
    let action = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra skills content <get|set> <id> [options]".to_string()))?;

    match action.as_str() {
        "get" => {
            let id = args
                .get(1)
                .filter(|a| !a.starts_with('-'))
                .ok_or_else(|| usage("usage: orquestra skills content get <id>".to_string()))?;
            let raw = read_skill_content(id.to_string()).map_err(CliError::Operation)?;
            Ok(raw)
        }
        "set" => {
            let id = args
                .get(1)
                .filter(|a| !a.starts_with('-'))
                .ok_or_else(|| usage("usage: orquestra skills content set <id> --file <path> [--json]".to_string()))?;
            let options = parse_options(&args[2..], &["--file"], &["--json"])?;
            let file_target = options.required("--file")?;

            let new_content = if file_target == "-" {
                let mut buffer = String::new();
                std::io::stdin().read_to_string(&mut buffer)
                    .map_err(|e| CliError::Operation(format!("failed to read stdin: {e}")))?;
                buffer
            } else {
                fs::read_to_string(&file_target)
                    .map_err(|e| CliError::Operation(format!("failed to read file '{file_target}': {e}")))?
            };

            save_skill_content(id.to_string(), new_content).map_err(CliError::Operation)?;
            let report = serde_json::json!({ "id": id, "updated": true });
            let human = format!("Updated SKILL.md for skill '{id}'.");
            rendered(&report, options.has("--json"), human)
        }
        other => Err(usage(format!("expected 'get' or 'set', got '{other}'"))),
    }
}
