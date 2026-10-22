//! 1-click updates: desktop binary replacement and official extension sync.
//!
//! Staging always lives under the OS temp scratch root
//! (`crate::paths::scratch_root`) and is removed on every exit path via a
//! [`Staging`] drop guard. Binary
//! downloads are fail-closed on minisign signature mismatch first (see
//! `update_sig`) and then on SHA-256 mismatch, and the live executable is
//! replaced atomically (sibling temp file + rename in the same directory).

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::Serialize;

const REPO: &str = "internetdozero/OrquestraAI";

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

/// Download `target_version` of the desktop binary, verify its SHA-256, and
/// atomically replace the running executable. Returns a human message.
#[tauri::command]
pub fn apply_app_update(target_version: String) -> Result<String, String> {
    let version = validate_version(&target_version)?;
    let assets = asset_candidates(std::env::consts::OS, std::env::consts::ARCH)?;
    let staging = Staging::fresh("orquestra-update")?;

    // Pin the tag the operator confirmed. `releases/latest` can move between
    // the UI check and this call, and the success string would still name
    // the older version.
    let mut asset = String::new();
    let mut binary_path = PathBuf::new();
    let mut last_error = String::new();
    for candidate in &assets {
        let path = staging.0.join(candidate);
        match download(&release_asset_url(&version, candidate), &path) {
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
    let sums_path = staging.0.join("SHA256SUMS.txt");
    download(&release_asset_url(&version, "SHA256SUMS.txt"), &sums_path)?;
    // Detached Ed25519 signature over the exact checksums bytes. A forged
    // release (malicious binary + matching malicious checksums) cannot forge
    // this without the offline signing key, so verify before trusting the sums.
    let sig_path = staging.0.join(crate::update_sig::SUMS_SIGNATURE_ASSET);
    download(
        &release_asset_url(&version, crate::update_sig::SUMS_SIGNATURE_ASSET),
        &sig_path,
    )?;
    let sums_bytes = fs::read(&sums_path)
        .map_err(|error| format!("failed to read {}: {error}", sums_path.display()))?;
    let sig_text = fs::read_to_string(&sig_path)
        .map_err(|error| format!("failed to read {}: {error}", sig_path.display()))?;
    crate::update_sig::verify_release_file(&sums_bytes, &sig_text)?;

    let expected = expected_hash(&sums_path, &asset)?;
    let actual = sha256_of(&binary_path)?;
    if expected != actual {
        return Err(format!(
            "checksum mismatch for {asset}: expected {expected}, got {actual}"
        ));
    }

    let target = active_binary()?;
    let sibling = target.with_file_name(format!(
        ".{}.tmp-{}",
        target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "orquestra".to_string()),
        std::process::id()
    ));
    fs::copy(&binary_path, &sibling)
        .map_err(|error| format!("failed to stage {}: {error}", sibling.display()))?;
    #[cfg(unix)]
    fs::set_permissions(&sibling, fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("failed to chmod {}: {error}", sibling.display()))?;
    fs::rename(&sibling, &target)
        .map_err(|error| format!("failed to replace {}: {error}", target.display()))?;

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
    let exe = active_binary()?;
    crate::single_instance::cleanup();
    std::process::Command::new(&exe)
        .spawn()
        .map_err(|error| format!("failed to relaunch {}: {error}", exe.display()))?;
    std::process::exit(0);
}

/// Running executable first, `~/.local/bin/orquestra` as fallback.
fn active_binary() -> Result<PathBuf, String> {
    if let Ok(exe) = std::env::current_exe() {
        if exe.is_file() {
            return Ok(exe);
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set; cannot locate the active binary".to_string())?;
    let fallback = home.join(".local/bin/orquestra");
    if fallback.is_file() {
        return Ok(fallback);
    }
    Err(
        "cannot locate the running binary (current_exe and ~/.local/bin/orquestra both missing)"
            .to_string(),
    )
}

/// Asset URL for one validated version tag (`v` is added back; validation strips it).
fn release_asset_url(version: &str, asset: &str) -> String {
    format!("https://github.com/{REPO}/releases/download/v{version}/{asset}")
}

/// Release asset candidates per platform; Linux keeps the legacy alias.
fn asset_candidates(os: &str, arch: &str) -> Result<Vec<&'static str>, String> {
    match (os, arch) {
        ("linux", "x86_64") => Ok(vec!["orquestra-linux-x86_64", "orquestra-ai"]),
        ("macos", "aarch64") => Ok(vec!["orquestra-macos-arm64"]),
        _ => Err(format!("unsupported platform for updates: {os}-{arch}")),
    }
}

/// Try each candidate asset in order until one downloads.
fn download(url: &str, destination: &Path) -> Result<(), String> {
    let output = std::process::Command::new("curl")
        .args(["-fsSL", "--retry", "2", "-o"])
        .arg(destination)
        .arg(url)
        .output()
        .map_err(|error| format!("curl is not available: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if detail.is_empty() {
        format!("download failed for {url:?}")
    } else {
        format!("download failed: {detail}")
    })
}

/// Exactly one `SHA256SUMS.txt` line must name `asset`; fail closed otherwise.
fn expected_hash(sums: &Path, asset: &str) -> Result<String, String> {
    let raw = fs::read_to_string(sums)
        .map_err(|error| format!("failed to read {}: {error}", sums.display()))?;
    let mut matches = Vec::new();
    for line in raw.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(hash), Some(name)) = (parts.next(), parts.next()) {
            let file = name
                .trim_start_matches('*')
                .rsplit('/')
                .next()
                .unwrap_or(name);
            if file == asset {
                matches.push(hash.to_lowercase());
            }
        }
    }
    match matches.as_slice() {
        [single] => Ok(single.clone()),
        [] => Err(format!("{asset:?} has no entry in SHA256SUMS.txt")),
        _ => Err(format!("{asset:?} has ambiguous checksum entries")),
    }
}

/// Hash a file with `sha256sum` (`shasum -a 256` fallback on macOS).
fn sha256_of(path: &Path) -> Result<String, String> {
    let text = path
        .to_str()
        .ok_or_else(|| "unrepresentable download path".to_string())?;
    for program in [&["sha256sum"][..], &["shasum", "-a", "256"][..]] {
        let Some((exe, args)) = program.split_first() else {
            continue;
        };
        let Ok(output) = std::process::Command::new(exe)
            .args(args)
            .arg(text)
            .output()
        else {
            continue;
        };
        if output.status.success() {
            if let Some(hash) = String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .next()
            {
                return Ok(hash.to_lowercase());
            }
        }
    }
    Err("no SHA-256 tool available (tried sha256sum, shasum)".to_string())
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
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_match_the_release_matrix() {
        assert_eq!(
            asset_candidates("linux", "x86_64").expect("linux"),
            vec!["orquestra-linux-x86_64", "orquestra-ai"]
        );
        assert_eq!(
            asset_candidates("macos", "aarch64").expect("macos"),
            vec!["orquestra-macos-arm64"]
        );
        assert!(asset_candidates("windows", "x86_64").is_err());
        assert!(asset_candidates("linux", "aarch64").is_err());
    }

    #[test]
    fn asset_url_pins_the_validated_release_tag() {
        let version = validate_version("v0.1.0-alpha.3").expect("tag");
        let binary = release_asset_url(&version, "orquestra-linux-x86_64");
        assert_eq!(
            binary,
            "https://github.com/internetdozero/OrquestraAI/releases/download/v0.1.0-alpha.3/orquestra-linux-x86_64"
        );
        assert!(!binary.contains("/releases/latest/"));
        let sums = release_asset_url(&version, "SHA256SUMS.txt");
        assert!(sums.ends_with("/releases/download/v0.1.0-alpha.3/SHA256SUMS.txt"));
    }

    #[test]
    fn versions_reject_paths_and_blanks() {
        assert_eq!(validate_version("v0.1.0").expect("tag"), "0.1.0");
        assert_eq!(
            validate_version("0.1.0-alpha.2").expect("pre"),
            "0.1.0-alpha.2"
        );
        for hostile in ["", "   ", "../escape", "a/b", "a\\b", "v with space"] {
            assert!(
                validate_version(hostile).is_err(),
                "{hostile:?} must be refused"
            );
        }
    }

    #[test]
    fn checksum_parsing_is_exact_and_unambiguous() {
        let dir = crate::paths::fresh_scratch_dir("orquestra-updater-test").expect("scratch");
        let sums = dir.join("SHA256SUMS.txt");
        fs::write(&sums, "abc123  orquestra-linux-x86_64\n Antenna\n").expect("write");
        assert_eq!(
            expected_hash(&sums, "orquestra-linux-x86_64").expect("hash"),
            "abc123"
        );
        assert!(expected_hash(&sums, "missing-binary").is_err());
        fs::write(
            &sums,
            "abc123  orquestra-linux-x86_64\ndef456 *orquestra-linux-x86_64\n",
        )
        .expect("rewrite");
        assert!(expected_hash(&sums, "orquestra-linux-x86_64").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn staging_uses_system_temp_scratch_root() {
        let staging = Staging::fresh("orquestra-update-test").expect("staging");
        let root = crate::paths::scratch_root();
        assert_eq!(root, std::env::temp_dir().join("orquestra-ai"));
        assert!(staging.0.starts_with(&root));
        // `Staging` removes the tree on drop.
    }
}
