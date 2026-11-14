#![cfg(target_os = "linux")]

//! Linux launcher and icon assets for [`crate::desktop`].
//!
//! Installs the `.desktop` entries and hicolor icons under `~/.local/share`
//! and refreshes the desktop caches. Everything here is symlink-aware: an
//! attacker-planted link must never receive launcher bytes, and cache
//! refreshes only fire when bytes actually changed.

use std::path::PathBuf;

const ICON_512: &[u8] = include_bytes!("../icons/icon.png");
const ICON_256: &[u8] = include_bytes!("../icons/128x128@2x.png");
const ICON_128: &[u8] = include_bytes!("../icons/128x128.png");
const ICON_64: &[u8] = include_bytes!("../icons/64x64.png");
const ICON_32: &[u8] = include_bytes!("../icons/32x32.png");

/// Launcher text with an explicit `Exec=`.
///
/// A bare `ownhub` only works when that name is on the menu session's
/// `PATH`. Running the AppImage from `~/Downloads`, or the binary from any
/// folder, wrote a launcher that did nothing when clicked.
pub(crate) fn desktop_entry(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Name=OwnHub\n\
         GenericName=AI Harness Control Plane\n\
         Comment=Universal AI Harness Administration Platform\n\
         Exec={exec} %U\n\
         Icon=ownhub\n\
         Terminal=false\n\
         Type=Application\n\
         Categories=Development;Utility;System;\n\
         StartupNotify=true\n\
         StartupWMClass=ownhub\n\
         Keywords=ai;harness;skills;mcp;agents;codex;claude;gemini;ownhub;\n"
    )
}

/// Absolute path of the running binary, quoted for a `Desktop Entry` file.
///
/// Falls back to the bare command only when the path is not representable;
/// that case is what the old launcher always did.
pub(crate) fn launch_exec() -> String {
    // An AppImage runs from a temporary mount that disappears on exit; the
    // path to keep is the `.AppImage` file itself, which `$APPIMAGE` names.
    let executable = std::env::var_os("APPIMAGE")
        .filter(|path| !path.is_empty())
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .map(|path| path.into_os_string())
        });
    executable
        .and_then(|path| path.into_string().ok())
        .map(|path| quote_exec(&path))
        .unwrap_or_else(|| "ownhub".to_string())
}

/// Quote an `Exec` value the way the Desktop Entry spec requires: only when
/// it contains a reserved character, and with `\\`, `\"`, `` ` `` and `$`
/// escaped inside the quotes.
fn quote_exec(path: &str) -> String {
    let reserved = "\"'\\><~|&;$*?#()`";
    let needs_quotes = path
        .chars()
        .any(|ch| ch.is_whitespace() || reserved.contains(ch));
    if !needs_quotes {
        return path.to_string();
    }
    let escaped = path
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    format!("\"{escaped}\"")
}

/// Launcher text for an alias entry: identical to the canonical entry but
/// hidden from menus. Without this, GNOME/KDE/XFCE list two or three
/// duplicate OwnHub icons (one per alias filename).
fn alias_entry(exec: &str) -> String {
    format!("{}NoDisplay=true\n", desktop_entry(exec))
}

/// Expected launcher bytes per filename: only `ownhub.desktop` is the
/// visible canonical entry; the technical aliases stay hidden. The
/// pre-OwnHub filenames are kept as hidden aliases so an existing install
/// converges them instead of leaving duplicate visible launchers.
fn entry_for(name: &str, exec: &str) -> Option<String> {
    match name {
        "ownhub.desktop" => Some(desktop_entry(exec)),
        "orquestra.desktop" | "orquestra-ai.desktop" | "ai.orquestra.desktop.desktop"
        | "dev.ownanai.hub.desktop" => Some(alias_entry(exec)),
        _ => None,
    }
}

/// Directory the XDG Base Directory spec calls the data home.
///
/// Hardcoding `~/.local/share` puts the launcher where the desktop
/// environment will never look whenever `XDG_DATA_HOME` is set (NixOS, some
/// immutability setups, sandboxed profiles). A relative or empty value is
/// ignored, which is what the spec says to do.
pub(crate) fn data_home() -> Option<PathBuf> {
    data_home_with(|name| std::env::var_os(name), crate::paths::home_dir())
}

/// The data home in the order the spec defines: an absolute
/// `XDG_DATA_HOME`, else `~/.local/share`.
pub(crate) fn data_home_with(
    lookup: impl Fn(&str) -> Option<std::ffi::OsString>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(configured) = lookup("XDG_DATA_HOME") {
        let path = PathBuf::from(configured);
        if path.is_absolute() {
            return Some(path);
        }
    }
    Some(home?.join(".local/share"))
}

/// Write the launcher and icons under `data_dir` (`~/.local/share`).
/// Reports whether any bytes changed so the caller can skip the desktop
/// refresh subprocesses when a launch changed nothing.
pub(crate) fn install_linux_assets(data_dir: &std::path::Path) -> bool {
    // Flatpak exports its own launcher pointing at `/app/bin/ownhub`,
    // which is meaningless outside the sandbox. Do not shadow it with a
    // host-visible entry that cannot launch.
    if crate::exec_env::in_flatpak() {
        return false;
    }
    let apps_dir = data_dir.join("applications");
    let icons_base = data_dir.join("icons/hicolor");
    let mut changed = false;
    let exec = launch_exec();

    if !is_symlink(&apps_dir) {
        let _ = std::fs::create_dir_all(&apps_dir);
    }
    for name in [
        "ownhub.desktop",
        "orquestra.desktop",
        "orquestra-ai.desktop",
        "ai.orquestra.desktop.desktop",
        "dev.ownanai.hub.desktop",
    ] {
        let dest = apps_dir.join(name);
        if is_symlink(&apps_dir) || is_symlink(&dest) {
            continue;
        }
        // A pre-dedup alias still carries the canonical text: rewrite it
        // hidden so one install pass converges every launcher.
        let Some(expected) = entry_for(name, &exec) else {
            continue;
        };
        let needs_write = match std::fs::read_to_string(&dest) {
            Ok(existing) => existing != expected,
            Err(_) => true,
        };
        if needs_write && std::fs::write(&dest, &expected).is_ok() {
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
        for icon_name in ["ownhub.png"] {
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
    for legacy_name in ["ownhub.png"] {
        let legacy = legacy_1024.join(legacy_name);
        if ancestor_is_symlink(data_dir, &legacy) || is_symlink(&legacy) {
            continue;
        }
        if std::fs::remove_file(&legacy).is_ok() {
            changed = true;
        }
    }

    for root_name in ["icons/ownhub.png"] {
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
    let icons = icons_base.to_string_lossy().into_owned();
    let apps = apps_dir.to_string_lossy().into_owned();

    let mut unavailable: Vec<&str> = Vec::new();
    if !run_quiet("gtk-update-icon-cache", &["-f", "-t", &icons]) {
        unavailable.push("gtk-update-icon-cache");
    }
    if !run_quiet("update-desktop-database", &[&apps]) {
        unavailable.push("update-desktop-database");
    }
    // Plasma 6 uses kbuildsycoca6, Plasma 5 uses kbuildsycoca5, and neither
    // exists outside KDE.
    if !run_quiet("kbuildsycoca6", &["--noincremental"])
        && !run_quiet("kbuildsycoca5", &["--noincremental"])
    {
        unavailable.push("kbuildsycoca");
    }

    if !unavailable.is_empty() {
        crate::log_warn!(
            "desktop cache refresh unavailable ({}); the launcher or icon may need a re-login to appear",
            unavailable.join(", ")
        );
    }
}

/// Run a cache refresher if it is installed; `false` means "not available or
/// failed", which the caller reports once.
fn run_quiet(program: &str, args: &[&str]) -> bool {
    let Some(path) = crate::exec_env::resolve(program) else {
        return false;
    };
    std::process::Command::new(path)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
