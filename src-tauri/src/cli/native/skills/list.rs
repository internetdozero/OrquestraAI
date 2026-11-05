//! Skills listing and summary subcommands.

use super::super::{parse_options, rendered, CliError};
use crate::skills::{harness_summary, list_skills, HarnessSummary, SkillItem};

pub(super) fn list(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--scope", "--filter"],
        &["--official", "--has-scripts", "--json"],
    )?;

    let scope = options.one("--scope")?.unwrap_or_else(|| "global".to_string());
    let filter = options.one("--filter")?.map(|s| s.to_lowercase());
    let official_only = options.has("--official");
    let has_scripts_only = options.has("--has-scripts");

    let mut skills = list_skills(Some(scope.clone())).map_err(CliError::Operation)?;

    if official_only {
        skills.retain(|s| s.official);
    }
    if has_scripts_only {
        skills.retain(|s| s.has_scripts);
    }
    if let Some(term) = filter {
        skills.retain(|s| {
            s.id.to_lowercase().contains(&term)
                || s.name.to_lowercase().contains(&term)
                || s.description.to_lowercase().contains(&term)
        });
    }

    let human = format_skills_table(&skills, &scope);
    rendered(&skills, options.has("--json"), human)
}

pub(super) fn summary(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(args, &[], &["--json"])?;
    let summaries = harness_summary().map_err(CliError::Operation)?;
    let human = format_summary_table(&summaries);
    rendered(&summaries, options.has("--json"), human)
}

fn format_skills_table(skills: &[SkillItem], scope: &str) -> String {
    if skills.is_empty() {
        return "No matching skills found.".to_string();
    }
    let mut lines = Vec::with_capacity(skills.len() + 1);
    lines.push(format!("{:<20} {:<24} {:<15} {:<10} {:<8} {}", "ID", "NAME", "CATEGORY", "SCOPE", "SCRIPTS", "OFFICIAL"));
    for s in skills {
        let scripts_str = if s.has_scripts { "yes" } else { "no" };
        let official_str = if s.official { "yes" } else { "no" };
        let enabled_str = if s.harness_enabled {
            if s.inherited_from_global && scope != "global" {
                "inherited"
            } else {
                "active"
            }
        } else {
            "inactive"
        };
        lines.push(format!(
            "{:<20} {:<24} {:<15} {:<10} {:<8} {}",
            s.id, s.name, s.category, enabled_str, scripts_str, official_str
        ));
    }
    lines.join("\n")
}

fn format_summary_table(summaries: &[HarnessSummary]) -> String {
    if summaries.is_empty() {
        return "No scopes found.".to_string();
    }
    let mut lines = Vec::with_capacity(summaries.len() + 1);
    lines.push(format!("{:<12} {:<8} {}", "SCOPE", "ACTIVE", "TOTAL"));
    for s in summaries {
        lines.push(format!("{:<12} {:<8} {}", s.id, s.active, s.total));
    }
    lines.join("\n")
}
