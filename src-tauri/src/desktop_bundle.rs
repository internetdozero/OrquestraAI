//! macOS desktop registration: an `OrquestraAI.app` bundle in `~/Applications`.
//!
//! macOS has no XDG launcher files. What makes an app appear in Launchpad,
//! Spotlight, the Dock and `open -a OrquestraAI` is a bundle: a directory with
//! `Contents/Info.plist` and an executable inside `Contents/MacOS`. The release
//! pipeline ships a raw Mach-O today (a real signed bundle is task #21), so the
//! app creates the bundle wrapper itself, once, on first launch.
//!
//! The executable inside the bundle is a **symlink** to the installed binary:
//! the updater replaces that binary in place, so a copy would go stale after
//! the first update, while the link always points at the current one.
//!
//! Pure file operations with no syscalls, so the logic is tested on any host.

const ICON_ICNS: &[u8] = include_bytes!("../icons/icon.icns");

/// Destination directory under the operator's home.
#[cfg(target_os = "macos")]
const APPS_DIR: &str = "Applications";
const BUNDLE_NAME: &str = "OrquestraAI.app";
const EXECUTABLE_NAME: &str = "orquestra";

/// `Info.plist` for the wrapper.
///
/// `CFBundleExecutable` must match the file name inside `Contents/MacOS`, and
/// `CFBundleIconFile` the `.icns` base name in `Contents/Resources` — a
/// mismatch gives a generic icon in the Dock.
pub(super) fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>OrquestraAI</string>
    <key>CFBundleDisplayName</key>
    <string>OrquestraAI</string>
    <key>CFBundleIdentifier</key>
    <string>ai.orquestra.desktop</string>
    <key>CFBundleExecutable</key>
    <string>{EXECUTABLE_NAME}</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
"#
    )
}

/// Create or refresh the bundle under `apps_dir`. Returns whether anything on
/// disk changed, so the caller can avoid pointless work on every launch.
pub(super) fn install_bundle(apps_dir: &std::path::Path, exe: &std::path::Path) -> bool {
    let contents = apps_dir.join(BUNDLE_NAME).join("Contents");
    let macos_dir = contents.join("MacOS");
    let resources = contents.join("Resources");
    if std::fs::create_dir_all(&macos_dir).is_err() || std::fs::create_dir_all(&resources).is_err()
    {
        return false;
    }

    let mut changed = false;

    let plist_path = contents.join("Info.plist");
    let plist = info_plist(env!("CARGO_PKG_VERSION"));
    let needs_plist = match std::fs::read_to_string(&plist_path) {
        Ok(existing) => existing != plist,
        Err(_) => true,
    };
    if needs_plist && std::fs::write(&plist_path, plist).is_ok() {
        changed = true;
    }

    let icon_path = resources.join("icon.icns");
    if std::fs::read(&icon_path)
        .map(|bytes| bytes != ICON_ICNS)
        .unwrap_or(true)
        && std::fs::write(&icon_path, ICON_ICNS).is_ok()
    {
        changed = true;
    }

    changed |= link_executable(&macos_dir.join(EXECUTABLE_NAME), exe);
    changed
}

/// Point the bundle's executable at the running binary.
///
/// A symlink is replaced rather than followed: `symlink` refuses an existing
/// path, and an existing link is only rewritten when it points elsewhere.
#[cfg(unix)]
fn link_executable(link: &std::path::Path, exe: &std::path::Path) -> bool {
    match std::fs::read_link(link) {
        Ok(current) if current == exe => return false,
        Ok(_) => {
            let _ = std::fs::remove_file(link);
        }
        Err(_) => {
            // Not a link: never overwrite a real file the operator put there.
            if std::fs::symlink_metadata(link).is_ok() {
                return false;
            }
        }
    }
    std::os::unix::fs::symlink(exe, link).is_ok()
}

/// Windows never reaches this module; the guard keeps the file compiling
/// where `std::os::unix` does not exist.
#[cfg(not(unix))]
fn link_executable(_link: &std::path::Path, _exe: &std::path::Path) -> bool {
    false
}

/// Entry point from [`crate::desktop`]: create the bundle for this install.
#[cfg(target_os = "macos")]
pub(crate) fn ensure_macos_bundle() {
    let Some(home) = crate::paths::home_dir() else {
        return;
    };
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if is_inside_bundle(&exe) {
        return;
    }
    install_bundle(&home.join(APPS_DIR), &exe);
}

/// A signed `OrquestraAI.app` already registers itself; wrapping it again
/// would produce a second, unsigned copy in the Dock.
///
/// The installer links `~/.local/bin/orquestra` at the executable inside the
/// bundle, and `current_exe` reports that link rather than its target, so the
/// resolved path is checked too.
pub(super) fn is_inside_bundle(exe: &std::path::Path) -> bool {
    let inside = |path: &std::path::Path| path.to_string_lossy().contains(".app/Contents/MacOS/");
    inside(exe)
        || std::fs::canonicalize(exe)
            .map(|real| inside(&real))
            .unwrap_or(false)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn sandbox() -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("orquestra-bundle-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn bundle_carries_plist_icon_and_a_link_to_the_binary() {
        let root = sandbox();
        let exe = root.join("orquestra-ai");
        std::fs::write(&exe, b"binary").unwrap();

        assert!(
            install_bundle(&root.join("Applications"), &exe),
            "first run writes"
        );
        let contents = root.join("Applications/OrquestraAI.app/Contents");
        let plist = std::fs::read_to_string(contents.join("Info.plist")).unwrap();
        assert!(
            plist.contains("<string>ai.orquestra.desktop</string>"),
            "{plist}"
        );
        assert!(plist.contains(&format!("<string>{}</string>", env!("CARGO_PKG_VERSION"))));
        assert_eq!(plist.matches("<key>CFBundleExecutable</key>").count(), 1);
        assert!(contents.join("Resources/icon.icns").is_file());

        #[cfg(unix)]
        assert_eq!(
            std::fs::read_link(contents.join("MacOS/").join(EXECUTABLE_NAME)).unwrap(),
            exe
        );

        // Second run over identical content must not report a change: the
        // desktop refresh is what makes a launch expensive.
        assert!(
            !install_bundle(&root.join("Applications"), &exe),
            "steady state"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A binary that already lives inside a signed `.app` must not be wrapped
    /// again, or the Dock shows the same app twice.
    #[test]
    fn a_bundled_binary_is_not_wrapped_twice() {
        assert!(is_inside_bundle(std::path::Path::new(
            "/Applications/OrquestraAI.app/Contents/MacOS/orquestra"
        )));
        assert!(!is_inside_bundle(std::path::Path::new(
            "/Users/dev/.local/bin/orquestra-ai"
        )));
    }

    /// The installer's `orquestra` command is a symlink into the bundle, and
    /// `current_exe` may report the link itself.
    #[cfg(unix)]
    #[test]
    fn a_cli_link_into_the_bundle_counts_as_bundled() {
        let root = sandbox();
        let bundle_bin = root.join("Applications/OrquestraAI.app/Contents/MacOS");
        std::fs::create_dir_all(&bundle_bin).unwrap();
        let exe = bundle_bin.join("orquestra-ai");
        std::fs::write(&exe, b"binary").unwrap();

        let shim = root.join("orquestra");
        std::os::unix::fs::symlink(&exe, &shim).unwrap();
        assert!(
            is_inside_bundle(&shim),
            "the CLI link resolves into the bundle"
        );
        assert!(is_inside_bundle(&exe));

        let _ = std::fs::remove_dir_all(&root);
    }
}
