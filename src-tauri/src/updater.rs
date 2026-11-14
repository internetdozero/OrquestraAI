//! 1-click updates: desktop binary replacement and official extension sync.
//!
//! Staging always lives under the secure runtime root
//! (`crate::paths::ensure_scratch_root`) and is removed on every exit path
//! via a [`Staging`] drop guard. Binary
//! downloads are fail-closed on minisign signature mismatch first (see
//! `update_sig`) and then on SHA-256 mismatch, and the live executable is
//! replaced atomically (sibling temp file + rename in the same directory).

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use tauri::Emitter;

mod download;
mod replace;

use download::{asset_candidates, download_async, expected_hash, release_asset_url, sha256_of};
pub use replace::purge_retired_binaries;

const UPDATE_PROGRESS_EVENT: &str = "app-update-progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateProgress {
    pub version: String,
    pub phase: &'static str,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Default)]
struct UpdateCoordinator {
    staged: Option<StagedBinary>,
    installed_version: Option<String>,
}

struct StagedBinary {
    version: String,
    binary_path: PathBuf,
    staging_dir: PathBuf,
}

static UPDATE_COORDINATOR: OnceLock<tauri::async_runtime::Mutex<UpdateCoordinator>> =
    OnceLock::new();

fn update_coordinator() -> &'static tauri::async_runtime::Mutex<UpdateCoordinator> {
    UPDATE_COORDINATOR.get_or_init(|| tauri::async_runtime::Mutex::new(UpdateCoordinator::default()))
}

fn emit_progress(
    app: Option<&tauri::AppHandle>,
    version: &str,
    phase: &'static str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) {
    if let Some(handle) = app {
        let _ = handle.emit(
            UPDATE_PROGRESS_EVENT,
            AppUpdateProgress {
                version: version.to_string(),
                phase,
                downloaded_bytes,
                total_bytes,
            },
        );
    }
}

/// Summary of an upstream sync of installed official extensions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionSyncResult {
    pub updated_skills: Vec<String>,
    pub updated_mcps: Vec<String>,
}

/// Scratch folder removed when it drops, so failures never leak downloads.
struct Staging(PathBuf);

impl Staging {
    fn fresh(prefix: &str) -> Result<Self, String> {
        Ok(Self(crate::paths::fresh_scratch_dir(prefix)?))
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Download caps: the binary is bounded at 100 MiB, the checksums at 1 MiB
/// and the detached signature at 64 KiB. Anything larger fails closed
/// instead of filling the staging disk.
const BINARY_MAX_BYTES: u64 = 100 * 1024 * 1024;
const SUMS_MAX_BYTES: u64 = 1024 * 1024;
const SIG_MAX_BYTES: u64 = 64 * 1024;

/// Download and verify `target_version` into a persistent staging directory.
/// Idempotent and single-flight: concurrent callers share the coordinator lock,
/// and an already-staged or already-installed version returns immediately.
#[tauri::command]
pub async fn download_app_update(
    app: tauri::AppHandle,
    target_version: String,
) -> Result<String, String> {
    let version = validate_version(&target_version)?;
    require_newer_version(&version)?;
    let mut guard = update_coordinator().lock().await;
    if guard.installed_version.as_deref() == Some(version.as_str()) {
        emit_progress(Some(&app), &version, "installed", 0, None);
        return Ok(version);
    }
    if let Some(staged) = &guard.staged {
        if staged.version == version && staged.binary_path.is_file() {
            emit_progress(Some(&app), &version, "ready", 0, None);
            return Ok(version);
        }
    }
    download_and_stage(Some(&app), &version, &mut guard).await?;
    Ok(version)
}

/// Atomically install a previously staged `target_version` (or download and
/// verify it first if not yet staged). Idempotent when already installed in
/// this session.
#[tauri::command]
pub async fn install_staged_update(
    app: tauri::AppHandle,
    target_version: String,
) -> Result<String, String> {
    let version = validate_version(&target_version)?;
    require_newer_version(&version)?;
    let mut guard = update_coordinator().lock().await;
    if guard.installed_version.as_deref() == Some(version.as_str()) {
        emit_progress(Some(&app), &version, "installed", 0, None);
        return Ok(format!("App updated successfully to {version}"));
    }
    let has_staged = guard
        .staged
        .as_ref()
        .is_some_and(|s| s.version == version && s.binary_path.is_file());
    if !has_staged {
        download_and_stage(Some(&app), &version, &mut guard).await?;
    }
    emit_progress(Some(&app), &version, "installing", 0, None);
    let staged = guard
        .staged
        .as_ref()
        .ok_or_else(|| "staged update missing after download".to_string())?;
    install_verified_binary(&staged.binary_path)?;
    let staging_dir = staged.staging_dir.clone();
    guard.staged = None;
    guard.installed_version = Some(version.clone());
    let _ = fs::remove_dir_all(&staging_dir);
    emit_progress(Some(&app), &version, "installed", 0, None);
    Ok(format!("App updated successfully to {version}"))
}

async fn download_and_stage(
    app: Option<&tauri::AppHandle>,
    version: &str,
    guard: &mut UpdateCoordinator,
) -> Result<(), String> {
    if let Some(old) = guard.staged.take() {
        let _ = fs::remove_dir_all(&old.staging_dir);
    }
    let staging_dir = crate::paths::ensure_scratch_root()?.join("staged-update");
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir_all(&staging_dir)
        .map_err(|error| format!("failed to create {}: {error}", staging_dir.display()))?;

    let result = download_and_verify_into(app, version, &staging_dir).await;
    match result {
        Ok(binary_path) => {
            guard.staged = Some(StagedBinary {
                version: version.to_string(),
                binary_path,
                staging_dir,
            });
            emit_progress(app, version, "ready", 0, None);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging_dir);
            Err(error)
        }
    }
}

async fn download_and_verify_into(
    app: Option<&tauri::AppHandle>,
    version: &str,
    staging_dir: &Path,
) -> Result<PathBuf, String> {
    let assets = asset_candidates(std::env::consts::OS, std::env::consts::ARCH)?;
    let mut asset = String::new();
    let mut binary_path = PathBuf::new();
    let mut last_error = String::new();
    emit_progress(app, version, "downloading", 0, None);
    for candidate in &assets {
        let path = staging_dir.join(candidate);
        match download_async(
            &release_asset_url(version, candidate),
            &path,
            BINARY_MAX_BYTES,
            std::time::Duration::from_secs(600),
            app.map(|handle| (handle, version)),
        )
        .await
        {
            Ok(()) => {
                asset = candidate.to_string();
                binary_path = path;
                break;
            }
            Err(error) => last_error = error,
        }
    }
    if asset.is_empty() {
        return Err(last_error);
    }
    emit_progress(app, version, "verifying", 0, None);
    let sums_path = staging_dir.join("SHA256SUMS.txt");
    download_async(
        &release_asset_url(version, "SHA256SUMS.txt"),
        &sums_path,
        SUMS_MAX_BYTES,
        std::time::Duration::from_secs(30),
        None,
    )
    .await?;
    let sig_path = staging_dir.join(crate::update_sig::SUMS_SIGNATURE_ASSET);
    download_async(
        &release_asset_url(version, crate::update_sig::SUMS_SIGNATURE_ASSET),
        &sig_path,
        SIG_MAX_BYTES,
        std::time::Duration::from_secs(30),
        None,
    )
    .await?;
    let sums_bytes = fs::read(&sums_path)
        .map_err(|error| format!("failed to read {}: {error}", sums_path.display()))?;
    let sig_text = fs::read_to_string(&sig_path)
        .map_err(|error| format!("failed to read {}: {error}", sig_path.display()))?;
    crate::update_sig::verify_release_file(&sums_bytes, &sig_text, version)?;

    let expected = expected_hash(&sums_path, &asset)?;
    let actual = sha256_of(&binary_path)?;
    if expected != actual {
        return Err(format!(
            "checksum mismatch for {asset}: expected {expected}, got {actual}"
        ));
    }
    Ok(binary_path)
}

fn install_verified_binary(binary_path: &Path) -> Result<(), String> {
    let target = replace::active_binary()?;
    let sibling = target.with_file_name(format!(
        ".{}.tmp-{}",
        target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "ownhub".to_string()),
        crate::paths::unique_tmp_suffix()
    ));
    fs::copy(binary_path, &sibling)
        .map_err(|error| format!("failed to stage {}: {error}", sibling.display()))?;
    #[cfg(unix)]
    fs::set_permissions(&sibling, fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("failed to chmod {}: {error}", sibling.display()))?;
    replace::install(&sibling, &target)
}

/// Download `target_version` of the desktop binary, verify its SHA-256, and
/// atomically replace the running executable. Returns a human message.
#[tauri::command]
pub fn apply_app_update(target_version: String) -> Result<String, String> {
    let version = validate_version(&target_version)?;
    require_newer_version(&version)?;
    let staging = Staging::fresh("ownhub-update")?;
    let binary_path = tauri::async_runtime::block_on(download_and_verify_into(
        None,
        &version,
        &staging.0,
    ))?;
    install_verified_binary(&binary_path)?;
    Ok(format!("App updated successfully to {version}"))
}

/// Pull upstream into every installed official skill with a known repo URL.
/// Vault folders are refreshed in place, so existing projection symlinks stay
/// live — no re-projection pass is needed.
#[tauri::command]
pub fn sync_official_extensions() -> Result<ExtensionSyncResult, String> {
    let paths = crate::paths::open_paths()?;
    let mut updated_skills = Vec::new();

    for skill in crate::official::official_store()
        .skills
        .iter()
        .filter(|skill| skill.official)
    {
        let Some(url) = skill.repo_url.as_deref() else {
            continue;
        };
        // Canonical vault folder first, then the legacy alias: pre-monorepo
        // installs keep syncing in place under their old folder name.
        let vault_dir = paths.vault.join(&skill.id);
        let vault_dir = if vault_dir.is_dir() {
            vault_dir
        } else if let Some(legacy) = skill.legacy_id.as_deref() {
            let legacy_dir = paths.vault.join(legacy);
            if legacy_dir.is_dir() {
                legacy_dir
            } else {
                continue;
            }
        } else {
            continue;
        };
        if crate::skills::refresh_skill_dir(&vault_dir, url, skill.fallback_repo_url.as_deref())
            .is_ok()
        {
            updated_skills.push(skill.id.clone());
        }
    }

    updated_skills.sort();
    Ok(ExtensionSyncResult {
        updated_skills,
        // Official MCPs ship as console-script packages the operator installs;
        // the catalog entry itself never drifts, so there is nothing to pull.
        updated_mcps: Vec::new(),
    })
}

/// Relaunch the (possibly just updated) binary and exit this process.
///
/// The single-instance socket file is removed *before* spawning: the child
/// checks for a live socket on startup, and `process::exit` runs no cleanup,
/// so spawning first lets the child connect to this still-dying parent and
/// terminate itself. Removing the file first makes the child the primary
/// instance deterministically — no delay needed, and the parent's still-open
/// listener fd does not block the child from binding the freed path.
#[tauri::command]
pub fn restart_app() -> Result<(), String> {
    let exe = replace::active_binary()?;
    crate::single_instance::cleanup();
    std::process::Command::new(&exe)
        .spawn()
        .map_err(|error| format!("failed to relaunch {}: {error}", exe.display()))?;
    std::process::exit(0);
}

/// Anti-replay/anti-downgrade gate: `target` must parse as SemVer and be
/// strictly greater than the running binary. Equal and lower versions fail
/// closed with a clear error before any network access. Prerelease ordering
/// follows SemVer, so `alpha.5 < alpha.10`.
fn require_newer_version(version: &str) -> Result<(), String> {
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|error| format!("running version is not valid SemVer: {error}"))?;
    let target = semver::Version::parse(version)
        .map_err(|_| format!("refusing to update to {version:?}: not a valid semantic version"))?;
    if target <= current {
        return Err(format!(
            "refusing to update to {target}: not newer than the running version {current} (downgrades and replays are refused)"
        ));
    }
    Ok(())
}

/// Non-empty dotted tag without any path semantics.
fn validate_version(raw: &str) -> Result<String, String> {
    let version = raw.trim().trim_start_matches(['v', 'V']).to_string();
    let ok = !version.is_empty()
        && version.len() <= 32
        && version
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-');
    if ok {
        Ok(version)
    } else {
        Err(format!("refusing to update to version {raw:?}"))
    }
}

#[cfg(test)]
mod tests;
