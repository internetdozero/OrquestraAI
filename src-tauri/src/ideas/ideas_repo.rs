//! Idea rows: create, list, update, delete.
//!
//! Same closed vocabularies, defaults and ordering as the Python MCP —
//! highest priority first, then newest.

use rusqlite::{params_from_iter, Connection};

use super::store::{
    db_error, get_idea, idea_from_row, optional_filter, require_choice, require_text,
};
use super::types::{CreateIdeaInput, IdeaItem, UpdateIdeaInput, CATEGORIES, PRIORITIES, STATUSES};

/// Highest priority first, mirroring the MCP's `PRIORITY_RANK`.
const PRIORITY_RANK: &str = "CASE priority WHEN 'critical' THEN 0 \
     WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END";


/// Insert one idea with MCP defaults (`feature` / `medium` / `open`).
pub fn create_idea(connection: &Connection, input: CreateIdeaInput) -> Result<IdeaItem, String> {
    let project = require_text(&input.project, "project")?;
    let title = require_text(&input.title, "title")?;
    let category = match input.category.as_deref() {
        None => "feature".to_string(),
        Some(raw) => require_choice(raw, &CATEGORIES, "category")?,
    };
    let priority = match input.priority.as_deref() {
        None => "medium".to_string(),
        Some(raw) => require_choice(raw, &PRIORITIES, "priority")?,
    };
    let description = input.description.map(|text| text.trim().to_string());
    let tags = input.tags.map(|text| text.trim().to_string());
    // Never store NULL: the column is NOT NULL DEFAULT '' (MCP parity),
    // so an absent prompt becomes "" and reads back as None.
    let prompt = input
        .prompt
        .map(|text| text.trim().to_string())
        .unwrap_or_default();

    connection
        .execute(
            "INSERT INTO ideas (project, title, description, category, priority, tags, prompt) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![project, title, description, category, priority, tags, prompt],
        )
        .map_err(db_error)?;

    get_idea(connection, connection.last_insert_rowid())
}

/// List ideas, highest priority first. A missing status defaults to `open`
/// (MCP parity); `"all"` disables the status filter.
pub fn list_ideas(
    connection: &Connection,
    project: Option<&str>,
    status: Option<&str>,
) -> Result<Vec<IdeaItem>, String> {
    let mut clauses = Vec::new();
    let mut params: Vec<String> = Vec::new();

    if let Some(raw) = project {
        if !raw.trim().is_empty() {
            clauses.push("project = ?");
            params.push(require_text(raw, "project")?);
        }
    }
    let status_filter = match status {
        None => Some("open".to_string()),
        Some(raw) => optional_filter(Some(raw), &STATUSES, "status")?,
    };
    if let Some(status) = status_filter {
        clauses.push("status = ?");
        params.push(status);
    }

    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {} ", clauses.join(" AND "))
    };
    let sql = format!(
        "SELECT * FROM ideas {where_sql}ORDER BY {PRIORITY_RANK}, created_at DESC, id DESC"
    );

    let mut statement = connection.prepare(&sql).map_err(db_error)?;
    let ideas = statement
        .query_map(params_from_iter(params.iter()), idea_from_row)
        .map_err(db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error)?;
    Ok(ideas)
}

/// Distinct project names for the filter dropdown.
pub fn list_idea_projects(connection: &Connection) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare("SELECT DISTINCT project FROM ideas ORDER BY project")
        .map_err(db_error)?;
    let projects = statement
        .query_map([], |row| row.get(0))
        .map_err(db_error)?
        .collect::<Result<Vec<String>, _>>()
        .map_err(db_error)?;
    Ok(projects)
}

/// Partial update: only `Some` fields are written; `updated_at` always moves.
pub fn update_idea(connection: &Connection, input: UpdateIdeaInput) -> Result<IdeaItem, String> {
    let mut sets: Vec<(&str, String)> = Vec::new();

    if let Some(raw) = input.project.as_deref() {
        sets.push(("project", require_text(raw, "project")?));
    }
    if let Some(raw) = input.title.as_deref() {
        sets.push(("title", require_text(raw, "title")?));
    }
    if let Some(raw) = input.description {
        sets.push(("description", raw.trim().to_string()));
    }
    if let Some(raw) = input.category.as_deref() {
        sets.push(("category", require_choice(raw, &CATEGORIES, "category")?));
    }
    if let Some(raw) = input.priority.as_deref() {
        sets.push(("priority", require_choice(raw, &PRIORITIES, "priority")?));
    }
    if let Some(raw) = input.tags {
        sets.push(("tags", raw.trim().to_string()));
    }
    if let Some(raw) = input.prompt {
        sets.push(("prompt", raw.trim().to_string()));
    }
    // `completed_at` follows the status: stamping a completion starts the
    // delivery clock for the heatmap, reopening clears it, and archiving
    // keeps whatever the idea held (a direct open -> archived jump was never
    // a delivery).
    let mut completion_stamp: Option<&str> = None;
    if let Some(raw) = input.status.as_deref() {
        let status = require_choice(raw, &STATUSES, "status")?;
        completion_stamp = match status.as_str() {
            "completed" => Some("completed_at = CURRENT_TIMESTAMP"),
            "open" | "in_progress" => Some("completed_at = NULL"),
            _ => None,
        };
        sets.push(("status", status));
    }

    let assignments = sets
        .iter()
        .map(|(column, _)| format!("{column} = ?"))
        .chain(std::iter::once("updated_at = CURRENT_TIMESTAMP".to_string()))
        .chain(completion_stamp.into_iter().map(str::to_string))
        .collect::<Vec<_>>()
        .join(", ");
    let values: Vec<String> = sets.into_iter().map(|(_, value)| value).collect();
    let id = input.id;

    // The id binds as an integer: no reliance on SQLite's implicit
    // TEXT-to-INTEGER affinity for the primary-key comparison.
    let changed = connection
        .execute(
            &format!("UPDATE ideas SET {assignments} WHERE id = ?"),
            params_from_iter(
                values
                    .iter()
                    .map(|value| value as &dyn rusqlite::ToSql)
                    .chain(std::iter::once(&id as &dyn rusqlite::ToSql)),
            ),
        )
        .map_err(db_error)?;
    if changed == 0 {
        return Err(format!("idea #{} not found", input.id));
    }

    get_idea(connection, input.id)
}

/// Move one idea to a new status.
pub fn update_idea_status(
    connection: &Connection,
    id: i64,
    status: &str,
) -> Result<IdeaItem, String> {
    update_idea(
        connection,
        UpdateIdeaInput {
            id,
            status: Some(status.to_string()),
            ..UpdateIdeaInput::default()
        },
    )
}

/// Delete one idea; a missing id is an error, never a silent no-op.
pub fn delete_idea(connection: &Connection, id: i64) -> Result<(), String> {
    let changed = connection
        .execute("DELETE FROM ideas WHERE id = ?1", [id])
        .map_err(db_error)?;
    if changed == 0 {
        return Err(format!("idea #{id} not found"));
    }
    Ok(())
}
