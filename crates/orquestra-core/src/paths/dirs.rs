//! Per-OS data and config directories, and the rebasing of harness paths.
//!
//! Every adapter used to build `home.join(".local/share/opencode")` and
//! `home.join(".config/manicode")` literally. That is correct on Linux and
//! wrong everywhere else: macOS keeps both under `~/Library/Application
//! Support`, Windows keeps them under `%LOCALAPPDATA%` and `%APPDATA%`, and a
//! Linux box with `XDG_DATA_HOME` set keeps them somewhere else entirely.
//!
//! Two layers, because callers differ:
//!
//! - [`data_dir`] / [`config_dir`] answer "where is this OS's data/config
//!   root for the current process" — for storage the app itself owns.
//! - [`rebase`] maps a home-relative harness path (`.config/opencode`, as it
//!   appears in the harness registry) onto the current platform while leaving
//!   paths that are the harness's own convention (`.claude`, `.agents`)
//!   untouched. Adapters receive the operator's home as an argument, so they
//!   need this form: it stays testable against a fake home.
//!
//! The Linux answers are exactly what the old literals produced, so a Linux
//! install sees no file move.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The XDG base directory Linux has used since 2010, and the prefix the
/// registry spells its XDG-relative paths with.
const XDG_DATA_REL: &str = ".local/share";
const XDG_CONFIG_REL: &str = ".config";

/// macOS has one application-support root for both roles.
const MACOS_SUPPORT_REL: &str = "Library/Application Support";

/// Windows defaults when the environment variable is missing, so a stripped
/// environment still resolves inside the operator's profile.
const WINDOWS_DATA_REL: &str = "AppData/Local";
const WINDOWS_CONFIG_REL: &str = "AppData/Roaming";

/// Data root for this process (`XDG_DATA_HOME`, `~/Library/Application
/// Support` or `%LOCALAPPDATA%`).
pub fn data_dir() -> Option<PathBuf> {
    data_dir_for(std::env::var_os("XDG_DATA_HOME"), home_dir(), os_name())
}

/// Config root for this process (`XDG_CONFIG_HOME`, `~/Library/Application
/// Support` or `%APPDATA%`).
pub fn config_dir() -> Option<PathBuf> {
    config_dir_for(std::env::var_os("XDG_CONFIG_HOME"), home_dir(), os_name())
}

/// Map a home-relative harness path onto this platform.
///
/// Only the two XDG prefixes are rewritten: `.claude`, `.agents`, `.copilot`
/// and friends are the harnesses' own conventions and stay beside the home
/// directory on every OS. Anything else is joined as before — which, for an
/// already-absolute path, `Path::join` returns unchanged.
pub fn rebase(home: &Path, rel: &str) -> PathBuf {
    rebase_for(
        home,
        rel,
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("XDG_CONFIG_HOME"),
        os_name(),
    )
}

/// [`data_dir`] with the environment and platform injected.
pub fn data_dir_for(
    xdg_data_home: Option<OsString>,
    home: Option<PathBuf>,
    os: &str,
) -> Option<PathBuf> {
    if let Some(absolute) = absolute_override(xdg_data_home) {
        return Some(absolute);
    }
    match os {
        "macos" => Some(home?.join(MACOS_SUPPORT_REL)),
        "windows" => Some(windows_root(home?, "LOCALAPPDATA", WINDOWS_DATA_REL)),
        _ => Some(home?.join(XDG_DATA_REL)),
    }
}

/// [`config_dir`] with the environment and platform injected.
pub fn config_dir_for(
    xdg_config_home: Option<OsString>,
    home: Option<PathBuf>,
    os: &str,
) -> Option<PathBuf> {
    if let Some(absolute) = absolute_override(xdg_config_home) {
        return Some(absolute);
    }
    match os {
        // Apple draws no distinction: preferences live beside application data.
        "macos" => Some(home?.join(MACOS_SUPPORT_REL)),
        "windows" => Some(windows_root(home?, "APPDATA", WINDOWS_CONFIG_REL)),
        _ => Some(home?.join(XDG_CONFIG_REL)),
    }
}

/// [`rebase`] with the environment and platform injected.
pub fn rebase_for(
    home: &Path,
    rel: &str,
    xdg_data_home: Option<OsString>,
    xdg_config_home: Option<OsString>,
    os: &str,
) -> PathBuf {
    if let Some(rest) = strip_prefix(rel, XDG_DATA_REL) {
        if let Some(root) = data_dir_for(xdg_data_home, Some(home.to_path_buf()), os) {
            return root.join(rest);
        }
    }
    if let Some(rest) = strip_prefix(rel, XDG_CONFIG_REL) {
        if let Some(root) = config_dir_for(xdg_config_home, Some(home.to_path_buf()), os) {
            return root.join(rest);
        }
    }
    home.join(rel)
}

/// `"<prefix>/rest"` → `"rest"`; the prefix itself → `""`; anything else →
/// `None`. Separators are normalised so a Windows-style rel also matches.
fn strip_prefix(rel: &str, prefix: &str) -> Option<String> {
    let normalised = rel.replace('\\', "/");
    let rest = normalised.strip_prefix(prefix)?;
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    Some(rest.to_string())
}

/// An XDG value is honoured only when absolute: the spec says relative values
/// are invalid, and treating them as home-relative silently moved files.
fn absolute_override(value: Option<OsString>) -> Option<PathBuf> {
    let path = PathBuf::from(value?);
    path.is_absolute().then_some(path)
}

/// `%VAR%` when set to an absolute path, else the profile default.
fn windows_root(home: PathBuf, var: &str, fallback_rel: &str) -> PathBuf {
    match absolute_override(std::env::var_os(var)) {
        Some(root) => root,
        None => home.join(fallback_rel),
    }
}

fn home_dir() -> Option<PathBuf> {
    crate::paths::home_dir()
}

/// `std::env::consts::OS` for the running platform.
fn os_name() -> &'static str {
    std::env::consts::OS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::DirEnv;

    /// The Windows roots read `%APPDATA%`/`%LOCALAPPDATA%` and Linux reads the
    /// XDG variables from the process, so these cases hold the shared directory
    /// lock and clear them: the Windows runner exports both and they otherwise
    /// answer with the real profile instead of the documented defaults.
    fn cleared() -> DirEnv {
        DirEnv::cleared()
    }

    /// Separators differ by host; the tests spell paths with `/`.
    fn shown(path: PathBuf) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    fn home() -> Option<PathBuf> {
        Some(PathBuf::from("/home/dev"))
    }

    const CASES: &[(&str, &str, &str)] = &[
        // os, data, config
        ("linux", "/home/dev/.local/share", "/home/dev/.config"),
        (
            "macos",
            "/home/dev/Library/Application Support",
            "/home/dev/Library/Application Support",
        ),
        (
            "windows",
            "/home/dev/AppData/Local",
            "/home/dev/AppData/Roaming",
        ),
    ];

    #[test]
    fn every_platform_has_its_documented_roots() {
        let _env = cleared();
        for (os, data, config) in CASES {
            assert_eq!(
                shown(data_dir_for(None, home(), os).expect("data root")),
                *data,
                "{os} data root"
            );
            assert_eq!(
                shown(config_dir_for(None, home(), os).expect("config root")),
                *config,
                "{os} config root"
            );
        }
    }

    /// A root the host itself calls absolute: `/nix/...` is not one on Windows,
    /// where an absolute path needs a drive or a UNC prefix.
    fn host_absolute(rel: &str) -> OsString {
        #[cfg(windows)]
        {
            OsString::from(format!(r"C:\{}", rel.replace('/', "\\")))
        }
        #[cfg(not(windows))]
        {
            OsString::from(format!("/{rel}"))
        }
    }

    #[test]
    fn xdg_overrides_win_only_when_absolute() {
        let _env = cleared();
        let profile = host_absolute("nix/store/profile/share");
        let xdg = host_absolute("etc/xdg-custom");
        let relative = Some(OsString::from("share"));
        assert_eq!(
            shown(data_dir_for(Some(profile.clone()), home(), "linux").expect("override")),
            shown(PathBuf::from(&profile))
        );
        assert_eq!(
            shown(config_dir_for(Some(xdg.clone()), home(), "linux").expect("override")),
            shown(PathBuf::from(&xdg))
        );
        assert_eq!(
            shown(data_dir_for(relative, home(), "linux").expect("fallback")),
            "/home/dev/.local/share",
            "a relative XDG value is invalid, not home-relative"
        );
    }

    #[test]
    fn a_missing_home_yields_no_root() {
        assert_eq!(data_dir_for(None, None, "linux"), None);
        assert_eq!(config_dir_for(None, None, "windows"), None);
    }

    #[test]
    fn rebase_rewrites_only_the_xdg_prefixes() {
        let home = Path::new("/home/dev");
        let linux = |rel: &str| rebase_for(home, rel, None, None, "linux");
        assert_eq!(
            linux(".local/share/opencode"),
            PathBuf::from("/home/dev/.local/share/opencode")
        );
        assert_eq!(
            linux(".config/kilo"),
            PathBuf::from("/home/dev/.config/kilo")
        );
        // The harnesses' own dotfolders are not XDG and must not move.
        assert_eq!(
            linux(".claude/skills"),
            PathBuf::from("/home/dev/.claude/skills")
        );
        assert_eq!(linux(".agents"), PathBuf::from("/home/dev/.agents"));
    }

    #[test]
    fn rebase_follows_the_platform_layout() {
        let _env = cleared();
        let home = Path::new("/Users/dev");
        assert_eq!(
            shown(rebase_for(home, ".config/manicode", None, None, "macos")),
            "/Users/dev/Library/Application Support/manicode"
        );
        assert_eq!(
            shown(rebase_for(
                home,
                ".local/share/opencode/opencode.db",
                None,
                None,
                "macos"
            )),
            "/Users/dev/Library/Application Support/opencode/opencode.db"
        );
        assert_eq!(
            shown(rebase_for(home, ".config/opencode", None, None, "windows")),
            "/Users/dev/AppData/Roaming/opencode"
        );
        assert_eq!(
            shown(rebase_for(home, ".local/share/kilo", None, None, "windows")),
            "/Users/dev/AppData/Local/kilo"
        );
    }

    /// A whole prefix with no remainder keeps the root itself, and a
    /// backslash-separated rel (as written in a Windows registry entry)
    /// rebases the same way.
    #[test]
    fn rebase_handles_bare_and_backslash_prefixes() {
        let home = Path::new("/home/dev");
        assert_eq!(
            rebase_for(home, ".config", None, None, "linux"),
            PathBuf::from("/home/dev/.config")
        );
        assert_eq!(
            rebase_for(home, ".config\\opencode", None, None, "linux"),
            PathBuf::from("/home/dev/.config/opencode")
        );
        assert_eq!(
            rebase_for(home, "/absolute/path", None, None, "linux"),
            PathBuf::from("/absolute/path"),
            "an absolute path is already resolved and must not be nested"
        );
    }
}
