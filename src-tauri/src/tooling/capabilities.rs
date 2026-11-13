//! Every external program the app shells out to, and what stops working
//! without it.
//!
//! The probe used to cover only `git`, so a machine without `python3-venv` or
//! `xdg-utils` reported "all present" while half the actions failed. Each row
//! here names the candidates, the features it unlocks, and the install command
//! the operator needs.

use std::ffi::OsStr;
use std::path::PathBuf;

use super::{ToolProbe, ToolReport};
use crate::exec_env;

/// Shown when the app runs inside a Flatpak sandbox: host tools are invisible
/// there no matter what the operator installed.
pub(super) const SANDBOX_NOTE: &str = "Running inside Flatpak: only tools shipped in the runtime are visible, so host installations cannot be used. Use the .deb/AppImage build for full functionality.";

/// How a capability proves itself beyond answering `--version`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Check {
    /// `--version` exits successfully.
    Banner,
    /// `--version` succeeds *and* the stdlib `venv` module can create an
    /// environment (Debian/Ubuntu ship it as a separate package).
    PythonVenv,
}

struct Capability {
    id: &'static str,
    candidates: &'static [&'static str],
    disables: &'static [&'static str],
    hint: &'static str,
    check: Check,
}

pub(super) fn report(search_path: Option<&OsStr>) -> ToolReport {
    let dirs: Option<Vec<PathBuf>> = search_path.map(|path| std::env::split_paths(path).collect());
    let tools: Vec<ToolProbe> = capabilities()
        .iter()
        .map(|capability| probe(capability, dirs.as_deref()))
        .collect();
    let sandboxed = exec_env::in_flatpak();
    ToolReport {
        missing: tools
            .iter()
            .filter(|tool| !tool.present)
            .map(|tool| tool.id.clone())
            .collect(),
        tools,
        sandboxed,
        note: sandboxed.then(|| SANDBOX_NOTE.to_string()),
    }
}

fn capabilities() -> Vec<Capability> {
    let mut rows = vec![
        Capability {
            id: "git",
            candidates: &["git"],
            disables: &["skill_sync", "skill_import"],
            hint: "Install git: pacman -S git | apt install git | xcode-select --install | winget install Git.Git",
            check: Check::Banner,
        },
        Capability {
            id: "python",
            candidates: python_candidates(),
            disables: &["mcp_install"],
            hint: "Install Python 3 with venv/ensurepip: apt install python3 python3-venv. On Windows use python.org, not the Microsoft Store alias.",
            check: Check::PythonVenv,
        },
        Capability {
            id: "node",
            candidates: &["node"],
            disables: &["mcp_presets"],
            hint: "Install Node.js 20+ (nvm, mise or the distro package) so npx-based presets can start.",
            check: Check::Banner,
        },
    ];
    // Windows opens links through ShellExecuteW and has no file-manager
    // launcher to probe.
    if !cfg!(windows) {
        rows.push(Capability {
            id: "opener",
            candidates: opener_candidates(),
            disables: &["open_url", "reveal_folder"],
            hint: "Install xdg-utils (xdg-open) or GLib's gio to open links and folders.",
            check: Check::Banner,
        });
    }
    rows
}

fn python_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["py", "python"]
    } else {
        &["python3"]
    }
}

fn opener_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["open"]
    } else {
        &["xdg-open", "gio"]
    }
}

fn probe(capability: &Capability, dirs: Option<&[PathBuf]>) -> ToolProbe {
    let resolved = capability.candidates.iter().find_map(|candidate| {
        let path = match dirs {
            Some(dirs) => exec_env::resolve_in(candidate, dirs),
            None => exec_env::resolve(candidate),
        }?;
        // The Microsoft Store alias for `python` is a stub that opens the
        // store instead of running anything.
        if cfg!(windows) && is_store_stub(&path) {
            return None;
        }
        Some(path)
    });

    let version = resolved.as_deref().and_then(|path| check(capability, path));
    let missing = version.is_none();
    ToolProbe {
        id: capability.id.to_string(),
        present: !missing,
        version,
        disables: capability
            .disables
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        hint: missing.then(|| capability.hint.to_string()),
    }
}

fn check(capability: &Capability, path: &std::path::Path) -> Option<String> {
    // macOS ships `/usr/bin/git` as a shim: running it with the Command Line
    // Tools missing opens the system installer dialog. Treat it as absent and
    // let the hint explain the install instead.
    #[cfg(target_os = "macos")]
    if capability.id == "git"
        && path == std::path::Path::new("/usr/bin/git")
        && !xcode_tools_installed()
    {
        return None;
    }

    let banner = banner(path)?;
    if capability.check == Check::PythonVenv && !python_venv_ready(path) {
        return None;
    }
    Some(banner)
}

fn banner(path: &std::path::Path) -> Option<String> {
    let mut command = std::process::Command::new(path);
    exec_env::hide_console(&mut command);
    let output = command.arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let first_line = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .lines()
            .next()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
    };
    first_line(&output.stdout).or_else(|| first_line(&output.stderr))
}

fn python_venv_ready(path: &std::path::Path) -> bool {
    for args in [vec!["-m", "venv", "--help"], vec!["-c", "import ensurepip"]] {
        let mut command = std::process::Command::new(path);
        exec_env::hide_console(&mut command);
        let ok = command
            .args(&args)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if !ok {
            return false;
        }
    }
    true
}

#[cfg(windows)]
fn is_store_stub(path: &std::path::Path) -> bool {
    path.to_string_lossy()
        .to_ascii_lowercase()
        .contains("windowsapps")
}

#[cfg(not(windows))]
fn is_store_stub(_path: &std::path::Path) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn xcode_tools_installed() -> bool {
    std::process::Command::new("/usr/bin/xcode-select")
        .arg("-p")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
        || std::path::Path::new("/Library/Developer/CommandLineTools/usr/bin/git").exists()
}
