//! Native desktop actions: opening URLs and folders in the system browser /
//! file manager, and snapshotting `~/.orquestra` into a real backup archive.
//!
//! The webview blocks `<a target="_blank">`, so external links and folder
//! reveals go through these commands. Every path is resolved against `HOME`
//! and validated before anything launches: `open_in_file_manager` only
//! reveals locations inside `~/.orquestra`, and non-`http(s)` URLs are
//! rejected so a crafted string can never escape into a local handler.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Root everything here is confined to.
fn orquestra_root() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set; cannot locate ~/.orquestra".to_string())?;
    Ok(home.join(".orquestra"))
}

/// Open an external HTTP(S) URL in the default system browser.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("https://") && !trimmed.starts_with("http://") {
        return Err("Only http/https URLs are permitted".to_string());
    }
    #[cfg(target_os = "linux")]
    let opener: (&str, Vec<&str>) = ("xdg-open", vec![trimmed]);
    #[cfg(target_os = "macos")]
    let opener: (&str, Vec<&str>) = ("open", vec![trimmed]);
    #[cfg(target_os = "windows")]
    let opener: (&str, Vec<&str>) = ("cmd", vec!["/C", "start", "", trimmed]);
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return Err("Opening URLs is not supported on this platform".to_string());
    std::process::Command::new(opener.0).args(&opener.1).spawn().map(|_| ())
        .map_err(|error| format!("Failed to open browser: {error}"))
}

/// Reveal a directory inside `~/.orquestra` in the native file manager.
/// `~` prefixes resolve against `HOME`; anything outside the root or that
/// does not exist is refused. Files reveal their parent folder.
#[tauri::command]
pub fn open_in_file_manager(path: String) -> Result<(), String> {
    let root = orquestra_root()?;
    let expanded = if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "HOME is not set".to_string())?;
        home.join(rest)
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
    let opener: (&str, Vec<&str>) = ("xdg-open", vec![reveal.to_str().ok_or("Unrepresentable path")?]);
    #[cfg(target_os = "macos")]
    let opener: (&str, Vec<&str>) = ("open", vec![reveal.to_str().ok_or("Unrepresentable path")?]);
    #[cfg(target_os = "windows")]
    let opener: (&str, Vec<&str>) = ("explorer", vec![reveal.to_str().ok_or("Unrepresentable path")?]);
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    return Err("Revealing folders is not supported on this platform".to_string());
    std::process::Command::new(opener.0).args(&opener.1).spawn().map(|_| ())
        .map_err(|error| format!("Failed to launch file manager: {error}"))
}

/// Snapshot `~/.orquestra` (configs, presets, vault, projections) into a
/// `tar.gz` under `~/.orquestra/backups/`, excluding previous backups.
/// Returns the archive path so the UI can toast the real destination.
#[tauri::command]
pub fn create_orquestra_backup() -> Result<String, String> {
    let root = orquestra_root()?;
    if !root.is_dir() {
        return Err(format!("{} does not exist yet", root.display()));
    }
    let backups = root.join("backups");
    std::fs::create_dir_all(&backups)
        .map_err(|error| format!("failed to create {}: {error}", backups.display()))?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let archive = backups.join(format!("orquestra_backup_{stamp}.tar.gz"));

    // Arg vector, no shell: nothing here is ever interpreted.
    let status = std::process::Command::new("tar")
        .args([
            "-czf",
            archive.to_str().ok_or("Unrepresentable backup path")?,
            "-C",
            root.to_str().ok_or("Unrepresentable root path")?,
            "--exclude=backups",
            ".",
        ])
        .status()
        .map_err(|error| format!("Tar execution failed: {error}"))?;
    if !status.success() {
        return Err("Failed to create archive".to_string());
    }

    Ok(archive.to_string_lossy().to_string())
}

const ICON_512: &[u8] = include_bytes!("../icons/icon.png");
const ICON_256: &[u8] = include_bytes!("../icons/128x128@2x.png");
const ICON_128: &[u8] = include_bytes!("../icons/128x128.png");
const ICON_64: &[u8] = include_bytes!("../icons/64x64.png");
const ICON_32: &[u8] = include_bytes!("../icons/32x32.png");

const DESKTOP_ENTRY: &str = r#"[Desktop Entry]
Name=OrquestraAI
GenericName=AI Harness Control Plane
Comment=Universal AI Harness Administration Platform
Exec=orquestra %U
Icon=orquestra
Terminal=false
Type=Application
Categories=Development;Utility;System;
StartupNotify=true
StartupWMClass=orquestra
Keywords=ai;harness;skills;mcp;agents;codex;claude;gemini;orquestra;
"#;

/// On Linux desktop environments, automatically install the .desktop launcher and
/// application icon theme assets so docks/taskbars (Wayland/X11) show the official icon.
#[cfg(target_os = "linux")]
pub fn ensure_desktop_integration() {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return;
    };
    let data_dir = home.join(".local/share");
    let apps_dir = data_dir.join("applications");
    let icons_base = data_dir.join("icons/hicolor");

    let _ = std::fs::create_dir_all(&apps_dir);
    for name in ["orquestra.desktop", "orquestra-ai.desktop"] {
        let dest = apps_dir.join(name);
        let needs_write = match std::fs::read_to_string(&dest) {
            Ok(existing) => existing != DESKTOP_ENTRY,
            Err(_) => true,
        };
        if needs_write {
            let _ = std::fs::write(&dest, DESKTOP_ENTRY);
        }
    }

    let icon_map: &[(&str, &[u8])] = &[
        ("32x32/apps", ICON_32),
        ("64x64/apps", ICON_64),
        ("128x128/apps", ICON_128),
        ("256x256/apps", ICON_256),
        ("512x512/apps", ICON_512),
    ];
    for (rel_dir, bytes) in icon_map {
        let target_dir = icons_base.join(rel_dir);
        let _ = std::fs::create_dir_all(&target_dir);
        for icon_name in ["orquestra.png", "orquestra-ai.png"] {
            let icon_file = target_dir.join(icon_name);
            let needs_write = match std::fs::read(&icon_file) {
                Ok(existing) => existing != *bytes,
                Err(_) => true,
            };
            if needs_write {
                let _ = std::fs::write(&icon_file, bytes);
            }
        }
    }

    let root_icon = data_dir.join("icons/orquestra.png");
    let needs_root_write = match std::fs::read(&root_icon) {
        Ok(existing) => existing != ICON_512,
        Err(_) => true,
    };
    if needs_root_write {
        let _ = std::fs::write(&root_icon, ICON_512);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn ensure_desktop_integration() {}
