//! Operations on ideas: list, get, add, update, status, delete, search, projects, export-md, metrics.

use super::super::{parse_i64, parse_options, parse_u32, rendered, usage, CliError};
use super::render::{render_idea_detail, render_ideas_table, render_metrics};
use crate::ideas::types::{CreateIdeaInput, UpdateIdeaInput};
use crate::ideas::{
    create_idea, delete_idea, export_ideas_markdown, get_idea, get_productivity_metrics,
    list_idea_projects, list_ideas, search_ideas, update_idea, update_idea_status,
};
use std::fs;

pub const LIST_USAGE: &str = "usage: ownhub idea list [--project <p>] [--status <status>] [--category <c>] [--priority <p>] [--limit <n>] [--offset <n>] [--json]\n  status: default is 'open'; use '--status all' to include completed and archived ideas";

pub(super) fn list(args: &[String]) -> Result<String, CliError> {
    if args
        .iter()
        .any(|arg| arg == "--help" || arg == "-h" || arg == "help")
    {
        return Err(usage(LIST_USAGE.to_string()));
    }

    let options = parse_options(
        args,
        &[
            "--project",
            "--status",
            "--category",
            "--priority",
            "--limit",
            "--offset",
        ],
        &["--json"],
    )?;

    let project = options.one("--project")?;
    let status = options.one("--status")?;
    let limit = parse_i64(&options, "--limit")?;
    let offset = parse_i64(&options, "--offset")?;

    let mut ideas = list_ideas(project, status, limit, offset).map_err(CliError::Operation)?;

    if let Some(cat) = options.one("--category")? {
        let needle = cat.to_lowercase();
        ideas.retain(|i| i.category.to_lowercase() == needle);
    }
    if let Some(prio) = options.one("--priority")? {
        let needle = prio.to_lowercase();
        ideas.retain(|i| i.priority.to_lowercase() == needle);
    }

    let human = render_ideas_table(&ideas);
    rendered(&ideas, options.has("--json"), human)
}

pub(super) fn get(args: &[String]) -> Result<String, CliError> {
    let id_str = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: ownhub idea get <id> [--json]".to_string()))?;

    let id = id_str
        .parse::<i64>()
        .map_err(|_| usage(format!("invalid idea ID '{id_str}', expected integer")))?;

    let options = parse_options(&args[1..], &[], &["--json"])?;
    let idea = get_idea(id).map_err(CliError::Operation)?;
    let human = render_idea_detail(&idea);
    rendered(&idea, options.has("--json"), human)
}

pub(super) fn add(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &[
            "--project",
            "--title",
            "--description",
            "--category",
            "--priority",
            "--tags",
            "--prompt",
            "--status",
        ],
        &["--json"],
    )?;

    let mut idea = create_idea(CreateIdeaInput {
        project: options.required("--project")?,
        title: options.required("--title")?,
        description: options.one("--description")?,
        category: options.one("--category")?,
        priority: options.one("--priority")?,
        tags: options.one("--tags")?,
        prompt: options.one("--prompt")?,
    })
    .map_err(CliError::Operation)?;

    if let Some(target_status) = options.one("--status")? {
        if target_status != "open" {
            idea = update_idea_status(idea.id, target_status).map_err(CliError::Operation)?;
        }
    }

    let human = format!(
        "Added #{} [{} / {}] {}: {}",
        idea.id, idea.priority, idea.status, idea.project, idea.title
    );
    rendered(&idea, options.has("--json"), human)
}

pub(super) fn update(args: &[String]) -> Result<String, CliError> {
    let id_str = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: ownhub idea update <id> [options] [--json]".to_string()))?;

    let id = id_str
        .parse::<i64>()
        .map_err(|_| usage(format!("invalid idea ID '{id_str}', expected integer")))?;

    let options = parse_options(
        &args[1..],
        &[
            "--title",
            "--project",
            "--description",
            "--category",
            "--priority",
            "--tags",
            "--prompt",
            "--status",
        ],
        &["--json"],
    )?;

    let idea = update_idea(UpdateIdeaInput {
        id,
        title: options.one("--title")?,
        project: options.one("--project")?,
        description: options.one("--description")?,
        category: options.one("--category")?,
        priority: options.one("--priority")?,
        tags: options.one("--tags")?,
        status: options.one("--status")?,
        prompt: options.one("--prompt")?,
    })
    .map_err(CliError::Operation)?;

    let human = format!(
        "Updated #{} [{} / {}] {}: {}",
        idea.id, idea.priority, idea.status, idea.project, idea.title
    );
    rendered(&idea, options.has("--json"), human)
}

pub(super) fn status(args: &[String]) -> Result<String, CliError> {
    let (id_str, status_str) = match args {
        [id, status] => (id.as_str(), status.as_str()),
        [id, status, flag] if flag == "--json" => (id.as_str(), status.as_str()),
        _ => {
            return Err(usage(
                "usage: ownhub idea status <id> <status> [--json]".to_string(),
            ))
        }
    };

    let id = id_str
        .parse::<i64>()
        .map_err(|_| usage(format!("invalid idea ID '{id_str}', expected integer")))?;

    let options = parse_options(&args[2..], &[], &["--json"])?;
    let idea = update_idea_status(id, status_str.to_string()).map_err(CliError::Operation)?;
    let human = format!("Idea #{} status changed to {}.", idea.id, idea.status);
    rendered(&idea, options.has("--json"), human)
}

pub(super) fn delete(args: &[String]) -> Result<String, CliError> {
    let id_str = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| usage("usage: ownhub idea delete <id> [--json]".to_string()))?;

    let id = id_str
        .parse::<i64>()
        .map_err(|_| usage(format!("invalid idea ID '{id_str}', expected integer")))?;

    let options = parse_options(&args[1..], &[], &["--json"])?;
    delete_idea(id).map_err(CliError::Operation)?;

    let report = serde_json::json!({ "id": id, "deleted": true });
    let human = format!("Deleted idea #{id}.");
    rendered(&report, options.has("--json"), human)
}

pub(super) fn search(args: &[String]) -> Result<String, CliError> {
    let query = args
        .first()
        .filter(|arg| !arg.starts_with('-'))
        .ok_or_else(|| {
            usage(
                "usage: ownhub idea search <query> [--project <p>] [--limit <n>] [--json]"
                    .to_string(),
            )
        })?;

    let options = parse_options(&args[1..], &["--project", "--limit"], &["--json"])?;
    let project = options.one("--project")?;
    let limit = parse_i64(&options, "--limit")?;

    let ideas = search_ideas(query, project, limit).map_err(CliError::Operation)?;
    let human = render_ideas_table(&ideas);
    rendered(&ideas, options.has("--json"), human)
}

pub(super) fn projects(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(args, &[], &["--json"])?;
    let projs = list_idea_projects().map_err(CliError::Operation)?;
    let human = if projs.is_empty() {
        "No projects found in Idea Vault.".to_string()
    } else {
        format!(
            "Projects ({}):\n{}",
            projs.len(),
            projs
                .iter()
                .map(|p| format!("  • {p}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    rendered(&projs, options.has("--json"), human)
}

pub(super) fn export_md(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(args, &["--project", "--out"], &["--json"])?;
    let project = options.required("--project")?;
    let md = export_ideas_markdown(&project).map_err(CliError::Operation)?;

    if let Some(out_path) = options.one("--out")? {
        fs::write(&out_path, &md).map_err(|e| {
            CliError::Operation(format!("failed to write markdown to '{out_path}': {e}"))
        })?;
        let report = serde_json::json!({ "project": project, "exportedPath": out_path });
        let human = format!("Exported ideas markdown for '{project}' to '{out_path}'.");
        return rendered(&report, options.has("--json"), human);
    }

    let report = serde_json::json!({ "project": project, "markdown": md });
    rendered(&report, options.has("--json"), md)
}

pub(super) fn metrics(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(args, &["--project", "--weeks"], &["--json"])?;
    let project = options.one("--project")?;
    let weeks = parse_u32(&options, "--weeks")?;

    let m = get_productivity_metrics(project, weeks).map_err(CliError::Operation)?;
    let human = render_metrics(&m);
    rendered(&m, options.has("--json"), human)
}
