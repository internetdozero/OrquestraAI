//! Native Idea Vault: data models, SQLite storage and IPC commands.
//!
//! The vault is shared with the `idea-vault` MCP, which keeps writing through
//! Python while these commands read and mutate the same rows. Every command
//! opens its own short-lived connection (5-second busy timeout), so neither
//! side ever blocks the other.

pub mod ideas_repo;
pub mod notes_repo;
pub mod productivity;
pub mod store;
#[cfg(test)]
mod tests;
pub mod types;

use types::{BlackboardNote, CreateIdeaInput, IdeaItem, PostNoteInput, UpdateIdeaInput};

/// Ideas, highest priority first; a missing status defaults to `open`.
/// `limit`/`offset` page the result (defaults and ceiling in
/// `store::clamp_page`); omitting them returns the default first page.
#[tauri::command]
pub fn list_ideas(
    project: Option<String>,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<IdeaItem>, String> {
    let connection = store::connect()?;
    ideas_repo::list_ideas(&connection, project.as_deref(), status.as_deref(), limit, offset)
}

/// Distinct project names for the filter dropdown.
#[tauri::command]
pub fn list_idea_projects() -> Result<Vec<String>, String> {
    let connection = store::connect()?;
    ideas_repo::list_idea_projects(&connection)
}

/// Record one idea with MCP defaults for omitted enums.
#[tauri::command]
pub fn create_idea(input: CreateIdeaInput) -> Result<IdeaItem, String> {
    let connection = store::connect()?;
    ideas_repo::create_idea(&connection, input)
}

/// Partial update; only `Some` fields are written.
#[tauri::command]
pub fn update_idea(input: UpdateIdeaInput) -> Result<IdeaItem, String> {
    let connection = store::connect()?;
    ideas_repo::update_idea(&connection, input)
}

/// Move one idea to a new status.
#[tauri::command]
pub fn update_idea_status(id: i64, status: String) -> Result<IdeaItem, String> {
    let connection = store::connect()?;
    ideas_repo::update_idea_status(&connection, id, &status)
}

/// Delete one idea; an unknown id is an error.
#[tauri::command]
pub fn delete_idea(id: i64) -> Result<(), String> {
    let connection = store::connect()?;
    ideas_repo::delete_idea(&connection, id)
}

/// Developer velocity: vault completions plus local git commits per day.
#[tauri::command]
pub fn get_productivity_metrics(
    project: Option<String>,
    weeks: Option<u32>,
) -> Result<productivity::ProductivityMetrics, String> {
    let connection = store::connect()?;
    productivity::metrics(&connection, project.as_deref(), weeks)
}

/// Blackboard notes oldest first, optionally unread only and/or one project.
/// Paged like ideas; omitting `limit`/`offset` returns the default page.
#[tauri::command]
pub fn list_blackboard_notes(
    project: Option<String>,
    unread_only: bool,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<BlackboardNote>, String> {
    let connection = store::connect()?;
    notes_repo::list_notes(&connection, project.as_deref(), unread_only, limit, offset)
}

/// Post a note for other agents; the topic is lowercased like the MCP does.
#[tauri::command]
pub fn post_blackboard_note(input: PostNoteInput) -> Result<BlackboardNote, String> {
    let connection = store::connect()?;
    notes_repo::post_note(&connection, input)
}

/// Mark one note read; an unknown id is an error.
#[tauri::command]
pub fn mark_note_read(id: i64) -> Result<(), String> {
    let connection = store::connect()?;
    notes_repo::mark_note_read(&connection, id)
}
