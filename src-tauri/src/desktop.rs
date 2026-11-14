//! Native desktop actions: opening URLs and folders in the system browser /
//! file manager, and snapshotting `~/.ownhub` into a real backup archive.
//!
//! The webview blocks `<a target="_blank">`, so external links and folder
//! reveals go through these commands. Every path is resolved against `HOME`
//! and validated before anything launches: `open_in_file_manager` only
//! reveals locations inside `~/.ownhub`, and non-`http(s)` URLs are
//! rejected so a crafted string can never escape into a local handler.

use std::path::{Path, PathBuf};

/// Root everything here is confined to.
fn ownhub_root() -> Result<PathBuf, String> {
    Ok(crate::paths::storage_root(&crate::paths::require_home(
        "locate ~/.ownhub",
    )?))
}

/// Open an external HTTP(S) URL in the default system browser.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let url = validate_url(&url)?;
    #[cfg(windows)]
    {
        // `ShellExecuteW` is an API call, not a process: there is no argv for a
        // shell to re-split, and it reports its own error code.
        return crate::windows_shell::open_url(&url);
    }
    #[cfg(not(windows))]
    {
        let candidates = browser_candidates(std::env::consts::OS, &url)?;
        spawn_first(candidates, "open a browser")
    }
}

/// Spawn the first launcher that is actually installed.
///
/// `xdg-open` lives in `xdg-utils`, which is not a hard dependency of
/// WebKitGTK/GTK and is absent from minimal Arch installs and containers. A
/// single hardcoded launcher therefore made every documentation link and
/// "reveal folder" action fail on a clean machine.
fn spawn_first(candidates: Vec<(&'static str, Vec<String>)>, action: &str) -> Result<(), String> {
    let mut tried: Vec<&'static str> = Vec::new();
    for (program, args) in candidates {
        let Some(path) = crate::exec_env::resolve(program) else {
            tried.push(program);
            continue;
        };
        return std::process::Command::new(&path)
            .args(&args)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Failed to {action} with {}: {error}", path.display()));
    }
    Err(format!(
        "Failed to {action}: none of {} is installed. {}",
        tried.join(", "),
        crate::exec_env::missing_message("xdg-open")
    ))
}

/// Reject anything that is not an http(s) URL without control characters.
///
/// The check is separate from the launcher because Windows no longer builds an
/// argv: `ShellExecuteW` would launch a `.exe` or hand the string to a
/// registered protocol handler just as readily as it opens a page.
fn validate_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("https://") && !trimmed.starts_with("http://") {
        return Err("Only http/https URLs are permitted".to_string());
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err("URL contains control characters".to_string());
    }
    Ok(trimmed.to_string())
}

/// Launcher candidates for one http(s) URL, best first.
///
/// The URL is a single argv entry, so nothing re-parses it. Windows is absent
/// on purpose: it uses `ShellExecuteW` in [`crate::windows_shell`], because
/// `cmd /C start` re-splits the command line on `&` and `rundll32` is a loader
/// Microsoft documents as not-for-direct-use.
#[cfg(any(not(windows), test))]
fn browser_candidates(os: &str, url: &str) -> Result<Vec<(&'static str, Vec<String>)>, String> {
    let owned = validate_url(url)?;
    match os {
        // `gio open` comes with GLib (always present with GTK); the rest cover
        // desktops that ship a different opener.
        "linux" => Ok(vec![
            ("gio", vec!["open".to_string(), owned.clone()]),
            ("xdg-open", vec![owned.clone()]),
            ("kde-open6", vec![owned.clone()]),
            ("kde-open5", vec![owned.clone()]),
            ("sensible-browser", vec![owned.clone()]),
            ("x-www-browser", vec![owned]),
        ]),
        "macos" => Ok(vec![("open", vec![owned])]),
        other => Err(format!(
            "Opening URLs is not supported on {other}; this build needs the platform shell API"
        )),
    }
}

/// Reveal a directory inside `~/.ownhub` in the native file manager.
/// `~` prefixes resolve against `HOME`; anything outside the root or that
/// does not exist is refused. Files reveal their parent folder.
#[tauri::command]
pub fn open_in_file_manager(path: String) -> Result<(), String> {
    let root = ownhub_root()?;
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
        return Err("Only paths inside ~/.ownhub can be opened".to_string());
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

    let candidates = file_manager_candidates(std::env::consts::OS, &target, &reveal)?;
    spawn_first(candidates, "launch a file manager")
}

/// Build the platform-specific reveal candidates without launching them.
fn file_manager_candidates(
    os: &str,
    target: &Path,
    reveal: &Path,
) -> Result<Vec<(&'static str, Vec<String>)>, String> {
    let path = |path: &Path| {
        path.to_str()
            .map(str::to_string)
            .ok_or_else(|| "Unrepresentable path".to_string())
    };
    match os {
        "linux" => {
            let folder = path(reveal)?;
            let item = path(target)?;
            Ok(vec![
                ("gio", vec!["open".to_string(), folder.clone()]),
                ("xdg-open", vec![folder.clone()]),
                ("nautilus", vec!["--select".to_string(), item.clone()]),
                ("dolphin", vec!["--select".to_string(), item.clone()]),
                ("thunar", vec![folder]),
            ])
        }
        "macos" => Ok(vec![("open", vec!["-R".to_string(), path(target)?])]),
        "windows" => Ok(vec![("explorer", vec![path(reveal)?])]),
        _ => Err("Revealing folders is not supported on this platform".to_string()),
    }
}

/// Snapshot `~/.ownhub` (configs, presets, vault, projections) into a
/// `tar.gz` under `~/.ownhub/backups/`, excluding previous backups.
/// Returns the archive path so the UI can toast the real destination.
/// Archive and directory are locked to owner-only (`0600`/`0700`) inside
/// [`crate::desktop_backup`]: the tarball embeds cleartext vault secrets.
#[tauri::command]
pub fn create_ownhub_backup() -> Result<String, String> {
    let root = ownhub_root()?;
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
/// caches; macOS writes an `OwnHub.app` bundle into `~/Applications`;
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
    use super::{browser_candidates, file_manager_candidates, validate_url};
    use std::path::Path;

    #[test]
    fn windows_launch_keeps_the_query_string_out_of_cmd() {
        // Windows does not build a command line at all: the URL goes to
        // `ShellExecuteW`, which cannot re-split a query string.
        assert!(browser_candidates("windows", "https://example.com/a?x=1").is_err());
        // Validation is shared, so a rejected URL never reaches the shell API.
        assert_eq!(
            validate_url("  https://example.com/a?x=1&y=2  ").expect("valid"),
            "https://example.com/a?x=1&y=2"
        );

        let candidates = browser_candidates("linux", "  http://example.com/docs  ").expect("http");
        assert_eq!(candidates[0].0, "gio");
        assert_eq!(
            candidates[0].1,
            vec!["open".to_string(), "http://example.com/docs".to_string()]
        );
        // The old single launcher is still tried, just not first.
        assert!(candidates.iter().any(|(program, _)| *program == "xdg-open"));

        assert!(browser_candidates("linux", "file:///etc/passwd").is_err());
        assert!(browser_candidates("windows", "https://example.com/\ncalc").is_err());
        assert!(browser_candidates("haiku", "https://example.com").is_err());
    }

    #[test]
    fn file_manager_reveals_the_item_in_finder_and_parent_on_other_platforms() {
        let target = Path::new("/home/user/.ownhub/config.json");
        let parent = Path::new("/home/user/.ownhub");

        let macos = file_manager_candidates("macos", target, parent).unwrap();
        assert_eq!(macos[0].0, "open");
        assert_eq!(
            macos[0].1,
            vec!["-R".to_string(), target.display().to_string()]
        );

        let linux = file_manager_candidates("linux", target, parent).unwrap();
        assert_eq!(linux[0].0, "gio");
        assert_eq!(
            linux[0].1,
            vec!["open".to_string(), parent.display().to_string()]
        );
        // Without GLib's gio, the XDG launcher remains available.
        assert!(linux.iter().any(|(program, _)| *program == "xdg-open"));

        let windows = file_manager_candidates("windows", target, parent).unwrap();
        assert_eq!(windows[0].0, "explorer");
        assert_eq!(windows[0].1, vec![parent.display().to_string()]);
        assert!(file_manager_candidates("haiku", target, parent).is_err());
    }
}
