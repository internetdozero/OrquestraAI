//! HTTP download transport and SHA-256 verification for app updates.

use std::fs;
use std::path::Path;

use super::emit_progress;

const REPO: &str = "OwnAnAI/ownhub";

/// Asset URL for one validated version tag (`v` is added back; validation strips it).
pub(super) fn release_asset_url(version: &str, asset: &str) -> String {
    format!("https://github.com/{REPO}/releases/download/v{version}/{asset}")
}

/// Release asset candidates per platform; Linux keeps the legacy alias.
pub(super) fn asset_candidates(os: &str, arch: &str) -> Result<Vec<&'static str>, String> {
    match (os, arch) {
        ("linux", "x86_64") => Ok(vec!["ownhub-linux-x86_64", "ownhub"]),
        ("linux", "aarch64") => Ok(vec!["ownhub-linux-aarch64"]),
        ("macos", "aarch64") => Ok(vec!["ownhub-macos-arm64"]),
        ("macos", "x86_64") => Ok(vec!["ownhub-macos-x86_64"]),
        // Windows ships the bare executable, like every other row: the MSI/NSIS
        // installer is for first install, and an in-place update must not run
        // one (it would relaunch the installer mid-session).
        ("windows", "x86_64") => Ok(vec!["ownhub-windows-x86_64.exe"]),
        _ => Err(format!("unsupported platform for updates: {os}-{arch}")),
    }
}

/// Try each candidate asset in order until one downloads.
///
/// Native `reqwest` (rustls) instead of a `curl` subprocess: minimal
/// containers and bare Windows installs have no curl in `PATH`, and the
/// release host redirects across origins, so this client keeps the default
/// redirect policy instead of the vault proxy's same-host-only one.
pub(super) async fn download_async(
    url: &str,
    destination: &Path,
    max_bytes: u64,
    timeout: std::time::Duration,
    progress: Option<(&tauri::AppHandle, &str)>,
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
        match download_once(&client, url, destination, max_bytes, progress).await {
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
    progress: Option<(&tauri::AppHandle, &str)>,
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
    let total_bytes = response.content_length();
    let mut file = fs::File::create(destination).map_err(|error| {
        fail(
            format!("failed to write {}: {error}", destination.display()),
            false,
        )
    })?;
    let mut total: u64 = 0;
    let mut last_emit: u64 = 0;
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
                if let Some((app, version)) = progress {
                    if total.saturating_sub(last_emit) >= 256 * 1024 || total_bytes == Some(total) {
                        last_emit = total;
                        emit_progress(Some(app), version, "downloading", total, total_bytes);
                    }
                }
            }
            Ok(None) => return Ok(()),
            Err(error) => {
                return Err(fail(format!("download failed for {url:?}: {error}"), true));
            }
        }
    }
}

/// Exactly one `SHA256SUMS.txt` line must name `asset`; fail closed otherwise.
pub(super) fn expected_hash(sums: &Path, asset: &str) -> Result<String, String> {
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
pub(super) fn sha256_of(path: &Path) -> Result<String, String> {
    use sha2::Digest;
    let mut file = fs::File::open(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut hasher = sha2::Sha256::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|error| format!("failed to hash {}: {error}", path.display()))?;
    Ok(format!("{:x}", hasher.finalize()))
}
