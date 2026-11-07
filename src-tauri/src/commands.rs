//! Async Tauri commands whose blocking work must not stall the UI thread.

use serde::Serialize;

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

/// Runtime identity of the native core, surfaced in the UI footer so the
/// frontend can prove the IPC bridge is alive.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreStatus {
    name: String,
    version: String,
    platform: String,
    bridge: String,
}

#[tauri::command]
pub(super) fn core_status(app: tauri::AppHandle) -> CoreStatus {
    let package = app.package_info();

    CoreStatus {
        name: package.name.clone(),
        version: package.version.to_string(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        bridge: "ready".to_string(),
    }
}

/// Crash reports from the React error boundaries land on stderr next to the
/// backend logs, so a white-screen report carries its area and message.
#[tauri::command]
pub(super) fn log_ui_error(
    area: String,
    message: String,
    stack: Option<String>,
) -> Result<(), String> {
    crate::log_error!("ui error in {area}: {message}");
    if let Some(stack) = stack.filter(|stack| !stack.is_empty()) {
        crate::log_error!("ui stack: {stack}");
    }
    Ok(())
}
