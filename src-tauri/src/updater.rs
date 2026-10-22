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

/// Download caps: the binary is bounded at 100 MiB, the checksums at 1 MiB
/// and the detached signature at 64 KiB. Anything larger fails closed
/// instead of filling the staging disk.
const BINARY_MAX_BYTES: u64 = 100 * 1024 * 1024;
const SUMS_MAX_BYTES: u64 = 1024 * 1024;
const SIG_MAX_BYTES: u64 = 64 * 1024;

/// Download `target_version` of the desktop binary, verify its SHA-256, and
/// atomically replace the running executable. Returns a human message.
#[tauri::command]
pub fn apply_app_update(target_version: String) -> Result<String, String> {
    let version = validate_version(&target_version)?;
    require_newer_version(&version)?;
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
        match download(
            &release_asset_url(&version, candidate),
            &path,
            BINARY_MAX_BYTES,
            std::time::Duration::from_secs(600),
        ) {
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
    download(
        &release_asset_url(&version, "SHA256SUMS.txt"),
        &sums_path,
        SUMS_MAX_BYTES,
        std::time::Duration::from_secs(30),
    )?;
    // Detached Ed25519 signature over the exact checksums bytes. A forged
    // release (malicious binary + matching malicious checksums) cannot forge
    // this without the offline signing key, so verify before trusting the sums.
    // The trusted comment binds the validated target version: a signature
    // lifted from an older release (replay/downgrade) names the wrong
    // version and fails closed even though it verifies cryptographically.
    let sig_path = staging.0.join(crate::update_sig::SUMS_SIGNATURE_ASSET);
    download(
        &release_asset_url(&version, crate::update_sig::SUMS_SIGNATURE_ASSET),
        &sig_path,
        SIG_MAX_BYTES,
        std::time::Duration::from_secs(30),
    )?;
    let sums_bytes = fs::read(&sums_path)
        .map_err(|error| format!("failed to read {}: {error}", sums_path.display()))?;
    let sig_text = fs::read_to_string(&sig_path)
        .map_err(|error| format!("failed to read {}: {error}", sig_path.display()))?;
    crate::update_sig::verify_release_file(&sums_bytes, &sig_text, &version)?;

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
        crate::paths::unique_tmp_suffix()
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
        ("macos", "x86_64") => Ok(vec!["orquestra-macos-x86_64"]),
        _ => Err(format!("unsupported platform for updates: {os}-{arch}")),
    }
}

/// Try each candidate asset in order until one downloads.
///
/// Native `reqwest` (rustls) instead of a `curl` subprocess: minimal
/// containers and bare Windows installs have no curl in `PATH`, and the
/// release host redirects across origins, so this client keeps the default
/// redirect policy instead of the vault proxy's same-host-only one.
fn download(
    url: &str,
    destination: &Path,
    max_bytes: u64,
    timeout: std::time::Duration,
) -> Result<(), String> {
    tauri::async_runtime::block_on(download_async(url, destination, max_bytes, timeout))
}

async fn download_async(
    url: &str,
    destination: &Path,
    max_bytes: u64,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(timeout)
        .build()
        .map_err(|error| format!("failed to build the HTTP client: {error}"))?;
    // Mirror `curl --retry 2`: transport failures retry, HTTP error
    // statuses (a missing candidate asset 404s by design) do not.
    let mut last_error = format!("download failed for {url:?}");
    for _ in 0..3 {
        match download_once(&client, url, destination, max_bytes).await {
            Ok(()) => return Ok(()),
            Err(error) if error.retryable => last_error = error.message,
            Err(error) => return Err(error.message),
        }
    }
    Err(last_error)
}

struct DownloadError {
    message: String,
    retryable: bool,
}

async fn download_once(
    client: &reqwest::Client,
    url: &str,
    destination: &Path,
    max_bytes: u64,
) -> Result<(), DownloadError> {
    let fail = |message: String, retryable: bool| DownloadError { message, retryable };
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|error| fail(format!("download failed for {url:?}: {error}"), true))?;
    let status = response.status();
    if !status.is_success() {
        return Err(fail(
            format!("download failed for {url:?}: HTTP {status}"),
            false,
        ));
    }
    let mut file = fs::File::create(destination).map_err(|error| {
        fail(
            format!("failed to write {}: {error}", destination.display()),
            false,
        )
    })?;
    let mut total: u64 = 0;
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                total = total.saturating_add(chunk.len() as u64);
                if total > max_bytes {
                    return Err(fail(
                        format!(
                            "download failed for {url:?}: response exceeds the {max_bytes}-byte cap"
                        ),
                        false,
                    ));
                }
                use std::io::Write;
                file.write_all(&chunk).map_err(|error| {
                    fail(
                        format!("failed to write {}: {error}", destination.display()),
                        false,
                    )
                })?;
            }
            Ok(None) => return Ok(()),
            Err(error) => {
                return Err(fail(format!("download failed for {url:?}: {error}"), true));
            }
        }
    }
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

/// Hash a file with SHA-256 from the `sha2` crate: no external
/// `sha256sum`/`shasum` needed, so minimal containers update fine.
fn sha256_of(path: &Path) -> Result<String, String> {
    use sha2::Digest;
    let mut file = fs::File::open(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut hasher = sha2::Sha256::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|error| format!("failed to hash {}: {error}", path.display()))?;
    Ok(format!("{:x}", hasher.finalize()))
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
        assert_eq!(
            asset_candidates("macos", "x86_64").expect("macos intel"),
            vec!["orquestra-macos-x86_64"]
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

    /// Task #325: equal and lower versions fail closed before any network
    /// access; prerelease ordering is numeric (`alpha.5 < alpha.10`).
    #[test]
    fn downgrades_and_replays_are_refused() {
        let current = env!("CARGO_PKG_VERSION");
        assert!(
            require_newer_version(current).is_err(),
            "re-applying the running version must be refused"
        );
        for older in ["0.1.0-alpha.4", "0.1.0-alpha.1", "0.0.9"] {
            let error =
                require_newer_version(older).expect_err(&format!("{older} must be refused"));
            assert!(error.contains("not newer"), "says why: {error}");
        }
        // The gate relies on numeric prerelease ordering, not string order.
        let v5 = semver::Version::parse("0.1.0-alpha.5").expect("v5");
        let v10 = semver::Version::parse("0.1.0-alpha.10").expect("v10");
        assert!(v5 < v10, "alpha.5 < alpha.10");
        assert!(require_newer_version("not-a-version").is_err());
        assert!(require_newer_version("").is_err());
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
    fn sha256_of_matches_the_nist_vector_without_external_tools() {
        let dir = crate::paths::fresh_scratch_dir("orquestra-updater-hash").expect("scratch");
        let file = dir.join("input.bin");
        fs::write(&file, b"abc").expect("write");
        assert_eq!(
            sha256_of(&file).expect("hash"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(sha256_of(&dir.join("missing.bin")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn staging_uses_the_secure_runtime_root() {
        let staging = Staging::fresh("orquestra-update-test").expect("staging");
        let root = crate::paths::ensure_scratch_root().expect("runtime root");
        assert_eq!(root, crate::paths::scratch_root());
        assert!(staging.0.starts_with(&root));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        // `Staging` removes the tree on drop.
    }
}
