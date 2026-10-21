#![cfg(target_os = "linux")]

//! Linux launcher and icon assets for [`crate::desktop`].
//!
//! Installs the `.desktop` entries and hicolor icons under `~/.local/share`
//! and refreshes the desktop caches. Everything here is symlink-aware: an
//! attacker-planted link must never receive launcher bytes, and cache
//! refreshes only fire when bytes actually changed.

const ICON_512: &[u8] = include_bytes!("../icons/icon.png");
const ICON_256: &[u8] = include_bytes!("../icons/128x128@2x.png");
const ICON_128: &[u8] = include_bytes!("../icons/128x128.png");
const ICON_64: &[u8] = include_bytes!("../icons/64x64.png");
const ICON_32: &[u8] = include_bytes!("../icons/32x32.png");

pub(crate) const DESKTOP_ENTRY: &str = r#"[Desktop Entry]
Name=OrquestraAI
GenericName=AI Harness Control Plane
Comment=Universal AI Harness Administration Platform
Exec=orquestra %U
Icon=orquestra-ai
Terminal=false
Type=Application
Categories=Development;Utility;System;
StartupNotify=true
StartupWMClass=orquestra
Keywords=ai;harness;skills;mcp;agents;codex;claude;gemini;orquestra;
"#;

/// Write the launcher and icons under `data_dir` (`~/.local/share`).
/// Reports whether any bytes changed so the caller can skip the desktop
/// refresh subprocesses when a launch changed nothing.
pub(crate) fn install_linux_assets(data_dir: &std::path::Path) -> bool {
    let apps_dir = data_dir.join("applications");
    let icons_base = data_dir.join("icons/hicolor");
    let mut changed = false;

    if !is_symlink(&apps_dir) {
        let _ = std::fs::create_dir_all(&apps_dir);
    }
    for name in ["orquestra.desktop", "orquestra-ai.desktop", "ai.orquestra.desktop.desktop"] {
        let dest = apps_dir.join(name);
        if is_symlink(&apps_dir) || is_symlink(&dest) {
            continue;
        }
        let needs_write = match std::fs::read_to_string(&dest) {
            Ok(existing) => existing != DESKTOP_ENTRY,
            Err(_) => true,
        };
        if needs_write && std::fs::write(&dest, DESKTOP_ENTRY).is_ok() {
            changed = true;
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
        if ancestor_is_symlink(data_dir, &target_dir) || is_symlink(&target_dir) {
            continue;
        }
        let _ = std::fs::create_dir_all(&target_dir);
        for icon_name in ["orquestra.png", "orquestra-ai.png"] {
            let icon_file = target_dir.join(icon_name);
            if is_symlink(&icon_file) {
                continue;
            }
            let needs_write = match std::fs::read(&icon_file) {
                Ok(existing) => existing != *bytes,
                Err(_) => true,
            };
            if needs_write && std::fs::write(&icon_file, bytes).is_ok() {
                changed = true;
            }
        }
    }

    // Clean up legacy 1024x1024 icons from early alpha builds that override theme colors
    let legacy_1024 = icons_base.join("1024x1024/apps");
    for legacy_name in ["orquestra.png", "orquestra-ai.png"] {
        let legacy = legacy_1024.join(legacy_name);
        if ancestor_is_symlink(data_dir, &legacy) || is_symlink(&legacy) {
            continue;
        }
        if std::fs::remove_file(&legacy).is_ok() {
            changed = true;
        }
    }

    for root_name in ["icons/orquestra.png", "icons/orquestra-ai.png"] {
        let root_icon = data_dir.join(root_name);
        if ancestor_is_symlink(data_dir, &root_icon) || is_symlink(&root_icon) {
            continue;
        }
        let needs_root_write = match std::fs::read(&root_icon) {
            Ok(existing) => existing != ICON_512,
            Err(_) => true,
        };
        if needs_root_write && std::fs::write(&root_icon, ICON_512).is_ok() {
            changed = true;
        }
    }

    changed
}

fn ancestor_is_symlink(root: &std::path::Path, path: &std::path::Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return true;
    };
    let mut current = root.to_path_buf();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        if components.peek().is_none() {
            break;
        }
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

fn is_symlink(path: &std::path::Path) -> bool {
    std::fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| metadata.file_type().is_symlink())
}

/// Ask the desktop environment to pick up new launchers and icons.
pub(crate) fn refresh_linux_desktop(data_dir: &std::path::Path) {
    let icons_base = data_dir.join("icons/hicolor");
    let apps_dir = data_dir.join("applications");
    let _ = std::process::Command::new("gtk-update-icon-cache")
        .args(["-f", "-t", &icons_base.to_string_lossy()])
        .output();
    let _ = std::process::Command::new("update-desktop-database")
        .arg(&apps_dir)
        .output();
    let _ = std::process::Command::new("kbuildsycoca6")
        .arg("--noincremental")
        .output();
}
