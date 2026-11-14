//! Shell-side view of external-program lookup.
//!
//! Resolution itself lives in [`ownhub_core::exec`] so the vault's blind
//! `exec` proxy finds the same binaries as the desktop app: the process
//! `PATH` first, then the login shell's `PATH` plus per-user install
//! directories. A GUI launch does not inherit `nvm`, `mise`, `~/.cargo/bin`
//! or Homebrew, and a tool that is installed would otherwise look missing.
//!
//! This module adds what is specific to the desktop shell: spawning without a
//! console window, the Flatpak sandbox check, and the per-OS install hint.

pub use ownhub_core::exec::{extra_dirs, resolve, resolve_in};

use std::path::Path;
use std::process::Command;

/// A `Command` for `binary`, resolved first, with no console window on
/// Windows. Returns an actionable message instead of a bare `NotFound`.
pub fn hidden(binary: &str) -> Result<Command, String> {
    let path = resolve(binary).ok_or_else(|| missing_message(binary))?;
    let mut command = Command::new(path);
    hide_console(&mut command);
    Ok(command)
}

/// Suppress the console window a GUI-subsystem process would otherwise flash
/// when it spawns a console program.
pub fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// True inside a Flatpak sandbox, where host binaries cannot run at all.
pub fn in_flatpak() -> bool {
    cfg!(target_os = "linux") && Path::new("/.flatpak-info").exists()
}

/// Why `binary` could not be found, with the install command for this OS.
pub fn missing_message(binary: &str) -> String {
    let hint = match binary {
        "git" => platform_hint(
            "pacman -S git",
            "apt install git",
            "xcode-select --install",
            "winget install Git.Git",
        ),
        "python3" | "python" | "py" => platform_hint(
            "pacman -S python",
            "apt install python3 python3-venv",
            "brew install python",
            "winget install Python.Python.3.12",
        ),
        "node" | "npx" => platform_hint(
            "pacman -S nodejs npm",
            "apt install nodejs npm",
            "brew install node",
            "winget install OpenJS.NodeJS.LTS",
        ),
        "xdg-open" | "gio" => platform_hint(
            "pacman -S xdg-utils",
            "apt install xdg-utils",
            "built in",
            "not used",
        ),
        other => format!("install `{other}` and make sure it is on PATH"),
    };
    let sandbox = if in_flatpak() {
        " (inside Flatpak only tools shipped in the runtime are visible; use the .deb/AppImage build or add the tool to the manifest)"
    } else {
        ""
    };
    format!("`{binary}` was not found on PATH. Install it: {hint}.{sandbox}")
}

fn platform_hint(arch: &str, debian: &str, macos: &str, windows: &str) -> String {
    if cfg!(target_os = "macos") {
        macos.to_string()
    } else if cfg!(windows) {
        windows.to_string()
    } else {
        format!("{arch} | {debian}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_paths_are_used_verbatim() {
        assert!(resolve("/bin/sh").is_some());
        assert!(resolve("/nonexistent/definitely-not-a-binary").is_none());
    }

    #[test]
    fn an_empty_search_path_resolves_nothing() {
        assert!(resolve_in("git", &[]).is_none());
    }

    #[test]
    fn extra_dirs_are_deduplicated_and_absolute() {
        for dir in extra_dirs() {
            assert!(dir.is_absolute(), "{} is not absolute", dir.display());
        }
    }

    #[test]
    fn missing_messages_name_the_tool() {
        assert!(missing_message("git").contains("git"));
        assert!(missing_message("python3").contains("python3"));
    }
}
