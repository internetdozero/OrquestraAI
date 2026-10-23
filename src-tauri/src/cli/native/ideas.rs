use super::{parse_i64, parse_options, rendered, usage, CliError};
use crate::ideas::types::CreateIdeaInput;

const USAGE: &str = "usage: orquestra ideas <add|list|status> [options]";

pub(super) fn run(args: &[String]) -> Result<String, CliError> {
    match args.first().map(String::as_str) {
        Some("add") => add(&args[1..]),
        Some("list") => list(&args[1..]),
        Some("status") => status(&args[1..]),
        _ => Err(usage(USAGE.to_string())),
    }
}

fn add(args: &[String]) -> Result<String, CliError> {
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
        ],
        &["--json"],
    )?;
    let idea = crate::ideas::create_idea(CreateIdeaInput {
        project: options.required("--project")?,
        title: options.required("--title")?,
        description: options.one("--description")?,
        category: options.one("--category")?,
        priority: options.one("--priority")?,
        tags: options.one("--tags")?,
        prompt: options.one("--prompt")?,
    })
    .map_err(CliError::Operation)?;
    let human = format!(
        "Added #{} [{} / {}] {}: {}",
        idea.id, idea.priority, idea.status, idea.project, idea.title
    );
    rendered(&idea, options.has("--json"), human)
}

fn list(args: &[String]) -> Result<String, CliError> {
    let options = parse_options(
        args,
        &["--project", "--status", "--limit", "--offset"],
        &["--json"],
    )?;
    let ideas = crate::ideas::list_ideas(
        options.one("--project")?,
        options.one("--status")?,
        parse_i64(&options, "--limit")?,
        parse_i64(&options, "--offset")?,
    )
    .map_err(CliError::Operation)?;
    let human = if ideas.is_empty() {
        "No ideas found.".to_string()
    } else {
        ideas
            .iter()
            .map(|idea| {
                format!(
                    "#{} [{} / {}] {}: {}",
                    idea.id, idea.priority, idea.status, idea.project, idea.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    rendered(&ideas, options.has("--json"), human)
}

fn status(args: &[String]) -> Result<String, CliError> {
    let mut positional = Vec::new();
    let mut json = false;
    for arg in args {
        if arg == "--json" {
            if json {
                return Err(usage("--json may only be supplied once".to_string()));
            }
            json = true;
        } else {
            positional.push(arg.as_str());
        }
    }
    if positional.len() != 2 {
        return Err(usage(
            "usage: orquestra ideas status <id> <status> [--json]".to_string(),
        ));
    }
    let id = positional[0]
        .parse::<i64>()
        .map_err(|_| usage("idea id must be an integer".to_string()))?;
    let idea = crate::ideas::update_idea_status(id, positional[1].to_string())
        .map_err(CliError::Operation)?;
    let human = format!("Idea #{} is now {}: {}", idea.id, idea.status, idea.title);
    rendered(&idea, json, human)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_requires_id_and_status_and_parses_id() {
        let missing = status(&[]);
        assert!(matches!(missing, Err(CliError::Usage(_))));

        let invalid = status(&["id".to_string(), "in_progress".to_string()]);
        assert!(matches!(invalid, Err(CliError::Usage(_))));
    }
}
