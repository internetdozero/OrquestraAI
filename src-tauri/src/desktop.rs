//! Native desktop actions: opening URLs and folders in the system browser /
//! file manager, and snapshotting `~/.orquestra` into a real backup archive.
//!
//! The webview blocks `<a target="_blank">`, so external links and folder
//! reveals go through these commands. Every path is resolved against `HOME`
//! and validated before anything launches: `open_in_file_manager` only
//! reveals locations inside `~/.orquestra`, and non-`http(s)` URLs are
//! rejected so a crafted string can never escape into a local handler.

use std::path::{Path, PathBuf};

/// Root everything here is confined to.
fn orquestra_root() -> Result<PathBuf, String> {
    Ok(crate::paths::require_home("locate ~/.orquestra")?.join(".orquestra"))
}

/// Open an external HTTP(S) URL in the default system browser.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let (program, args) = browser_launch(std::env::consts::OS, &url)?;
    std::process::Command::new(program)
        .args(&args)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to open browser: {error}"))
}

/// Launcher for one http(s) URL. The URL is a single argv entry.
///
/// Windows must not use `cmd /C start`: cmd re-parses the command line, so a
/// query string (`?a=1&b=2`) is split on `&` and the tail runs as a command.
fn browser_launch(os: &str, url: &str) -> Result<(&'static str, Vec<String>), String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("https://") && !trimmed.starts_with("http://") {
        return Err("Only http/https URLs are permitted".to_string());
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err("URL contains control characters".to_string());
    }
    let owned = trimmed.to_string();
    match os {
        "linux" => Ok(("xdg-open", vec![owned])),
        "macos" => Ok(("open", vec![owned])),
        "windows" => Ok((
            "rundll32",
            vec!["url.dll,FileProtocolHandler".to_string(), owned],
        )),
        _ => Err("Opening URLs is not supported on this platform".to_string()),
    }
}

/// Reveal a directory inside `~/.orquestra` in the native file manager.
/// `~` prefixes resolve against `HOME`; anything outside the root or that
/// does not exist is refused. Files reveal their parent folder.
#[tauri::command]
pub fn open_in_file_manager(path: String) -> Result<(), String> {
    let root = orquestra_root()?;
    let expanded = if let Some(rest) = path.strip_prefix("~/") {
        crate::paths::require_home("expand ~")?.join(rest)
    } else {
        PathBuf::from(path)
    };

    let target = expanded
        .canonicalize()
        .map_err(|_| format!("Path does not exist: {}", expanded.display()))?;
    let canonical_root = root
        .canonicalize()
        .map_err(|_| format!("{} does not exist yet", root.display()))?;
    if !target.starts_with(&canonical_root) {
        return Err("Only paths inside ~/.orquestra can be opened".to_string());
    }

    // Revealing means showing the folder: a file resolves to its parent.
    let reveal = if target.is_file() {
        target
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| target.clone())
    } else {
        target.clone()
    };

    let (program, args) = file_manager_launch(std::env::consts::OS, &target, &reveal)?;
    std::process::Command::new(program)
        .args(&args)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to launch file manager: {error}"))
}

/// Build the platform-specific reveal command without launching it.
fn file_manager_launch(
    os: &str,
    target: &Path,
    reveal: &Path,
) -> Result<(&'static str, Vec<String>), String> {
    let path = |path: &Path| {
        path.to_str()
            .map(str::to_string)
            .ok_or_else(|| "Unrepresentable path".to_string())
    };
    match os {
        "linux" => Ok(("xdg-open", vec![path(reveal)?])),
        "macos" => Ok(("open", vec!["-R".to_string(), path(target)?])),
        "windows" => Ok(("explorer", vec![path(reveal)?])),
        _ => Err("Revealing folders is not supported on this platform".to_string()),
    }
}

/// Snapshot `~/.orquestra` (configs, presets, vault, projections) into a
/// `tar.gz` under `~/.orquestra/backups/`, excluding previous backups.
/// Returns the archive path so the UI can toast the real destination.
/// Archive and directory are locked to owner-only (`0600`/`0700`) inside
/// [`crate::desktop_backup`]: the tarball embeds cleartext vault secrets.
#[tauri::command]
pub fn create_orquestra_backup() -> Result<String, String> {
    let root = orquestra_root()?;
    if !root.is_dir() {
        return Err(format!("{} does not exist yet", root.display()));
    }
    crate::desktop_backup::create_backup_at(&root)
        .map(|archive| archive.to_string_lossy().to_string())
}

/// Linux: write the assets, then refresh the caches only when something
/// changed.
#[cfg(target_os = "linux")]
fn refresh_integration_once() {
    let Some(data_dir) = crate::desktop_icons::data_home() else {
        return;
    };
    if crate::desktop_icons::install_linux_assets(&data_dir) {
        crate::desktop_icons::refresh_linux_desktop(&data_dir);
    }
}

/// macOS: no launcher files exist in XDG terms, so the app registers itself as
/// a bundle in `~/Applications` (Dock, Spotlight and `open -a` all find it).
#[cfg(target_os = "macos")]
fn refresh_integration_once() {
    crate::desktop_bundle::ensure_macos_bundle();
}

/// Windows: a Start Menu shortcut, which is what the Start menu, search and
/// `Win+R` index.
#[cfg(target_os = "windows")]
fn refresh_integration_once() {
    crate::desktop_shortcut::ensure_start_menu_shortcut();
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn ensure_desktop_integration() {}

/// Register the app with the current desktop, once per launch.
///
/// Linux installs `.desktop` entries and hicolor icons and refreshes the
/// caches; macOS writes an `OrquestraAI.app` bundle into `~/Applications`;
/// Windows writes a Start Menu shortcut. All of it runs on a detached worker
/// thread so the first window never waits on file writes or cache refreshes,
/// and every write is content-compared first, so a steady-state launch costs
/// one thread spawn.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn ensure_desktop_integration() {
    let worker = std::thread::Builder::new().name("desktop-integration".to_string());
    if worker.spawn(refresh_integration_once).is_err() {
        refresh_integration_once();
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "desktop_icons_tests.rs"]
mod icon_tests;

#[cfg(test)]
mod tests {
    use super::{browser_launch, file_manager_launch};
    use std::path::Path;

    #[test]
    fn windows_launch_keeps_the_query_string_out_of_cmd() {
        let (program, args) =
            browser_launch("windows", "https://example.com/a?x=1&y=2").expect("https");
        assert_eq!(program, "rundll32");
        assert_eq!(args[0], "url.dll,FileProtocolHandler");
        assert_eq!(args[1], "https://example.com/a?x=1&y=2");
        assert!(args
            .iter()
            .all(|arg| arg != "/C" && arg != "cmd" && arg != "start"));

        let (program, args) = browser_launch("linux", "  http://example.com/docs  ").expect("http");
        assert_eq!(program, "xdg-open");
        assert_eq!(args, vec!["http://example.com/docs".to_string()]);

        assert!(browser_launch("linux", "file:///etc/passwd").is_err());
        assert!(browser_launch("windows", "https://example.com/\ncalc").is_err());
        assert!(browser_launch("haiku", "https://example.com").is_err());
    }

    #[test]
    fn file_manager_reveals_the_item_in_finder_and_parent_on_other_platforms() {
        let target = Path::new("/home/user/.orquestra/config.json");
        let parent = Path::new("/home/user/.orquestra");

        assert_eq!(
            file_manager_launch("macos", target, parent).unwrap(),
            ("open", vec!["-R".to_string(), target.display().to_string()])
        );
        assert_eq!(
            file_manager_launch("linux", target, parent).unwrap(),
            ("xdg-open", vec![parent.display().to_string()])
        );
        assert_eq!(
            file_manager_launch("windows", target, parent).unwrap(),
            ("explorer", vec![parent.display().to_string()])
        );
        assert!(file_manager_launch("haiku", target, parent).is_err());
    }
}
