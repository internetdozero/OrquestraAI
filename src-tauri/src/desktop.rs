//! Native desktop actions: opening URLs and folders in the system browser /
//! file manager, and snapshotting `~/.orquestra` into a real backup archive.
//!
//! The webview blocks `<a target="_blank">`, so external links and folder
//! reveals go through these commands. Every path is resolved against `HOME`
//! and validated before anything launches: `open_in_file_manager` only
//! reveals locations inside `~/.orquestra`, and non-`http(s)` URLs are
//! rejected so a crafted string can never escape into a local handler.

use std::path::PathBuf;

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
        target.parent().map(PathBuf::from).unwrap_or(target)
    } else {
        target
    };

    #[cfg(target_os = "linux")]
    let opener: (&str, Vec<&str>) = (
        "xdg-open",
        vec![reveal.to_str().ok_or("Unrepresentable path")?],
    );
    #[cfg(target_os = "macos")]
    let opener: (&str, Vec<&str>) = ("open", vec![reveal.to_str().ok_or("Unrepresentable path")?]);
    #[cfg(target_os = "windows")]
    let opener: (&str, Vec<&str>) = (
        "explorer",
        vec![reveal.to_str().ok_or("Unrepresentable path")?],
    );
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return Err("Revealing folders is not supported on this platform".to_string());
    std::process::Command::new(opener.0)
        .args(&opener.1)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to launch file manager: {error}"))
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

/// On Linux desktop environments, automatically install the .desktop launcher and
/// application icon theme assets so docks/taskbars (Wayland/X11) show the official icon.
///
/// Runs off the setup path: a detached worker thread performs the file writes
/// and the icon-cache refresh, so the first window never waits on
/// `gtk-update-icon-cache` / `update-desktop-database` / `kbuildsycoca6`.
/// The refresh subprocesses only fire when launcher or icon bytes actually
/// changed, so steady-state launches cost one thread spawn and a cheap
/// content comparison.
#[cfg(target_os = "linux")]
pub fn ensure_desktop_integration() {
    let worker = std::thread::Builder::new().name("desktop-integration".to_string());
    if worker.spawn(refresh_integration_once).is_err() {
        // Threading is best-effort like every write below: a missed refresh
        // only delays icon pickup until the next launch.
        refresh_integration_once();
    }
}

/// Install assets, then refresh caches only when something changed.
#[cfg(target_os = "linux")]
fn refresh_integration_once() {
    let Some(home) = crate::paths::home_dir() else {
        return;
    };
    let data_dir = home.join(".local/share");
    if crate::desktop_icons::install_linux_assets(&data_dir) {
        crate::desktop_icons::refresh_linux_desktop(&data_dir);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn ensure_desktop_integration() {}

#[cfg(test)]
mod tests {
    use super::browser_launch;

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

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_install_does_not_write_through_a_symlink() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::path::PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-desktop-link-{}-{unique}",
            std::process::id()
        ));
        let data = root.join("share");
        std::fs::create_dir_all(data.join("applications")).unwrap();
        let outside = root.join("outside.txt");
        let original = "ssh-private\n";
        std::fs::write(&outside, original).unwrap();
        let link = data.join("applications/orquestra.desktop");
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        crate::desktop_icons::install_linux_assets(&data);

        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the desktop entry symlink must stay"
        );
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), original);
        let regular =
            std::fs::read_to_string(data.join("applications/orquestra-ai.desktop")).unwrap();
        assert!(regular.contains("Name=OrquestraAI"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_install_does_not_write_into_a_symlinked_applications_dir() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::path::PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-desktop-dir-link-{}-{unique}",
            std::process::id()
        ));
        let data = root.join("share");
        let outside = root.join("outside");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let link = data.join("applications");
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        crate::desktop_icons::install_linux_assets(&data);

        assert!(
            !outside.join("orquestra.desktop").exists(),
            "install must not create a launcher in the symlink target"
        );
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Steady-state launches must not fire the icon-cache subprocesses: the
    /// install reports a change only on the first run, so the refresh gate
    /// in `refresh_integration_once` stays shut afterwards.
    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_install_reports_change_only_once() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let data = std::path::PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-desktop-idem-{}-{unique}",
            std::process::id()
        ));

        assert!(
            crate::desktop_icons::install_linux_assets(&data),
            "first install writes launcher and icons"
        );
        assert_eq!(
            std::fs::read_to_string(data.join("applications/orquestra.desktop")).unwrap(),
            crate::desktop_icons::DESKTOP_ENTRY,
        );
        assert!(
            !crate::desktop_icons::install_linux_assets(&data),
            "nothing changed, so no cache refresh is warranted"
        );

        let _ = std::fs::remove_dir_all(&data);
    }

    /// Only the canonical launcher is menu-visible: the technical aliases
    /// carry `NoDisplay=true`, so GNOME/KDE/XFCE show a single OrquestraAI
    /// icon. A pre-dedup alias (canonical text from an older install)
    /// converges to hidden on the next run.
    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_alias_entries_stay_hidden_from_menus() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let data = std::path::PathBuf::from("/var/tmp/testing").join(format!(
            "orquestra-desktop-nodisplay-{}-{unique}",
            std::process::id()
        ));

        assert!(crate::desktop_icons::install_linux_assets(&data));
        let canonical =
            std::fs::read_to_string(data.join("applications/orquestra.desktop")).unwrap();
        assert!(!canonical.contains("NoDisplay"), "canonical stays visible");
        for alias in ["orquestra-ai.desktop", "ai.orquestra.desktop.desktop"] {
            let text = std::fs::read_to_string(data.join(format!("applications/{alias}"))).unwrap();
            assert!(text.contains("Name=OrquestraAI"));
            assert!(
                text.lines().any(|line| line == "NoDisplay=true"),
                "{alias} must stay hidden from menus"
            );
        }

        // An older install left the canonical text in the alias: one pass
        // converges it to hidden, then the install is steady again.
        std::fs::write(
            data.join("applications/orquestra-ai.desktop"),
            crate::desktop_icons::DESKTOP_ENTRY,
        )
        .unwrap();
        assert!(
            crate::desktop_icons::install_linux_assets(&data),
            "pre-dedup alias must converge to hidden"
        );
        let converged =
            std::fs::read_to_string(data.join("applications/orquestra-ai.desktop")).unwrap();
        assert!(converged.lines().any(|line| line == "NoDisplay=true"));
        assert!(
            !crate::desktop_icons::install_linux_assets(&data),
            "steady state must not rewrite"
        );

        let _ = std::fs::remove_dir_all(&data);
    }
}
