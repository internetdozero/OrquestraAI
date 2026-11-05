//! Skill inspect, info, and validate subcommands.

use std::fs;
use std::path::{Path, PathBuf};
use super::super::{parse_options, rendered, usage, CliError};
use crate::harness::harness_ids;
use crate::paths::OrquestraPaths;
use crate::skills::list_skills;

pub(super) fn inspect(args: &[String]) -> Result<String, CliError> {
    let id = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra skills info <id> [--json]".to_string()))?;

    let options = parse_options(&args[1..], &[], &["--json"])?;

    let global_skills = list_skills(Some("global".to_string())).map_err(CliError::Operation)?;
    let skill = global_skills
        .into_iter()
        .find(|s| s.id == *id)
        .ok_or_else(|| CliError::Operation(format!("unknown skill: {id}")))?;

    let mut projection_matrix = Vec::new();
    let global_active = skill.global_enabled;
    projection_matrix.push(("global".to_string(), if global_active { "ENABLED" } else { "DISABLED" }));

    for harness in harness_ids() {
        let scoped_list = list_skills(Some(harness.to_string())).map_err(CliError::Operation)?;
        let status = if let Some(item) = scoped_list.into_iter().find(|s| s.id == *id) {
            if item.inherited_from_global {
                "ENABLED (inherited from global)"
            } else if item.harness_enabled {
                "ENABLED"
            } else {
                "DISABLED"
            }
        } else {
            "DISABLED"
        };
        projection_matrix.push((harness.to_string(), status));
    }

    let report = serde_json::json!({
        "skill": skill,
        "projections": projection_matrix.iter().map(|(h, s)| serde_json::json!({ "scope": h, "status": s })).collect::<Vec<_>>(),
    });

    let mut scripts_lines = Vec::new();
    if skill.script_files.is_empty() {
        scripts_lines.push("  (None detected)".to_string());
    } else {
        for f in &skill.script_files {
            scripts_lines.push(format!("  • {f}"));
        }
    }

    let mut matrix_lines = Vec::new();
    for (scope, status) in &projection_matrix {
        matrix_lines.push(format!("  - {:<10} {}", format!("{scope}:"), status));
    }

    let human = format!(
        "Skill:              {}\n\
         Name:               {}\n\
         Category:           {}\n\
         Official:           {}\n\
         Tokens:             ~{}\n\
         Has Scripts:        {}\n\
         Scripts Detected ({} files):\n{}\n\n\
         Projection Matrix:\n{}",
        skill.id,
        skill.name,
        skill.category,
        if skill.official { "yes" } else { "no" },
        skill.tokens,
        if skill.has_scripts { "yes" } else { "no" },
        skill.script_files.len(),
        scripts_lines.join("\n"),
        matrix_lines.join("\n")
    );

    rendered(&report, options.has("--json"), human)
}

pub(super) fn validate(args: &[String]) -> Result<String, CliError> {
    let target = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| usage("usage: orquestra skills validate <path|id> [--json]".to_string()))?;

    let options = parse_options(&args[1..], &[], &["--json"])?;

    let paths = OrquestraPaths::resolve().map_err(|e| CliError::Operation(e.to_string()))?;
    let path = if Path::new(target).exists() {
        PathBuf::from(target)
    } else {
        paths.vault.join(target)
    };

    let mut errors = Vec::new();

    if !path.is_dir() {
        errors.push(format!("path '{}' is not an existing directory", path.display()));
    }

    let skill_file = path.join("SKILL.md");
    let mut frontmatter_name = None;
    let mut frontmatter_desc = None;

    if !skill_file.is_file() {
        errors.push("missing required SKILL.md file".to_string());
    } else {
        match fs::read_to_string(&skill_file) {
            Ok(content) => {
                let trimmed = content.trim_start();
                if trimmed.starts_with("---") {
                    if let Some(end) = trimmed[3..].find("---") {
                        let yaml_block = &trimmed[3..3 + end];
                        for line in yaml_block.lines() {
                            if let Some((k, v)) = line.split_once(':') {
                                let key = k.trim();
                                let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
                                if key == "name" && !val.is_empty() {
                                    frontmatter_name = Some(val);
                                } else if key == "description" && !val.is_empty() {
                                    frontmatter_desc = Some(val);
                                }
                            }
                        }
                    }
                }
                if frontmatter_name.is_none() {
                    errors.push("SKILL.md frontmatter missing 'name'".to_string());
                }
                if frontmatter_desc.is_none() {
                    errors.push("SKILL.md frontmatter missing 'description'".to_string());
                }
            }
            Err(e) => errors.push(format!("failed to read SKILL.md: {e}")),
        }
    }

    let is_valid = errors.is_empty();
    let report = serde_json::json!({
        "target": target,
        "path": path.display().to_string(),
        "valid": is_valid,
        "errors": errors,
        "name": frontmatter_name,
        "description": frontmatter_desc,
    });

    if !is_valid {
        let err_msg = format!("Validation failed for '{}':\n{}", target, errors.iter().map(|e| format!("  • {e}")).collect::<Vec<_>>().join("\n"));
        if options.has("--json") {
            return rendered(&report, true, err_msg);
        }
        return Err(CliError::Operation(err_msg));
    }

    let human = format!("Validation passed for '{}' ({}).", target, path.display());
    rendered(&report, options.has("--json"), human)
}
