//! Async Tauri commands whose blocking work must not stall the UI thread.

use crate::skills::{self, SkillItem};

/// Shallow-clone a git URL and import the skill it holds without blocking IPC.
#[tauri::command]
pub(super) async fn import_skill_from_git(url: String) -> Result<SkillItem, String> {
    tauri::async_runtime::spawn_blocking(move || skills::import_skill_from_git(url))
        .await
        .map_err(|error| format!("skill import task failed: {error}"))?
}
