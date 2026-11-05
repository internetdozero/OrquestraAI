//! Async Tauri commands whose blocking work must not stall the UI thread.

use crate::skills::{self, SkillItem};

pub(super) async fn run_blocking<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| format!("background task failed: {error}"))?
}

/// Shallow-clone a git URL and import the skill it holds without blocking IPC.
#[tauri::command]
pub(super) async fn import_skill_from_git(url: String) -> Result<SkillItem, String> {
    run_blocking(move || skills::import_skill_from_git(url)).await
}
