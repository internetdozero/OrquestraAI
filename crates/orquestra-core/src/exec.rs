//! Resolve external programs from a process that may not have a shell's
//! `PATH`.
//!
//! A desktop app started from a menu or launcher does not inherit the `PATH` a
//! terminal gets from `~/.bashrc` / `~/.zshrc`. Version managers (`nvm`,
//! `mise`, `asdf`) and per-user installs (`~/.local/bin`, `~/.cargo/bin`,
//! Homebrew) are therefore invisible, and a tool that is installed looks
//! missing.
//!
//! Resolution order: the process `PATH` first (cheap, and never surprises an
//! operator who launched the binary from a shell), then a cached set of extra
//! directories discovered once per process — the login shell's `PATH` plus the
//! conventional per-user install locations. The login shell runs with a short
//! deadline: a broken rc file must not hang the first lookup.
//!
//! Windows has no login shell: the user `PATH` is read from
//! `HKCU\Environment`, which is what an installer updates and a new process
//! would otherwise only see after a re-login.
//!
//! Shared by the desktop shell (`exec_env`) and the vault's blind `exec`
//! proxy, so both find the same binaries.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// How long a login shell may take to print its `PATH`.
const SHELL_TIMEOUT: Duration = Duration::from_secs(2);

/// Directories worth trying when the process `PATH` does not answer.
pub fn extra_dirs() -> &'static [PathBuf] {
    static EXTRA: OnceLock<Vec<PathBuf>> = OnceLock::new();
    EXTRA.get_or_init(compute_extra_dirs)
}

fn compute_extra_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    #[cfg(unix)]
    if let Some(path) = login_shell_path() {
        dirs.extend(std::env::split_paths(&path));
    }

    #[cfg(windows)]
    dirs.extend(windows_user_path());

    if let Some(home) = crate::paths::home_dir() {
        for relative in [
            ".local/bin",
            ".cargo/bin",
            ".local/share/mise/shims",
            ".asdf/shims",
        ] {
            dirs.push(home.join(relative));
        }
    }
    // Homebrew on Apple Silicon and Intel. Harmless elsewhere (a missing
    // directory is skipped).
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));

    dedupe(dirs)
}

fn dedupe(dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut unique: Vec<PathBuf> = Vec::with_capacity(dirs.len());
    for dir in dirs {
        if dir.as_os_str().is_empty() {
            continue;
        }
        if !unique.contains(&dir) {
            unique.push(dir);
        }
    }
    unique
}

/// The `PATH` a login shell would export, bounded by [`SHELL_TIMEOUT`].
#[cfg(unix)]
fn login_shell_path() -> Option<std::ffi::OsString> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;

    let shell = std::env::var_os("SHELL")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| std::ffi::OsString::from("/bin/sh"));
    // Only an absolute path: a relative `$SHELL` from a mangled environment
    // would resolve against the app's cwd.
    if !Path::new(&shell).is_absolute() {
        return None;
    }

    let mut child = Command::new(&shell)
        .args(["-lc", "printf %s \"$PATH\""])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;

    // Read on a worker so a shell that never exits cannot block the deadline
    // check; killing the child closes the pipe and ends the reader.
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        let _ = sender.send(text);
    });

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < SHELL_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let text = receiver.recv_timeout(Duration::from_millis(200)).ok()?;
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(std::ffi::OsString::from(text))
    }
}

/// `Path` from `HKCU\Environment`, without pulling a registry crate in.
#[cfg(windows)]
fn windows_user_path() -> Vec<PathBuf> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let Ok(output) = std::process::Command::new("reg")
        .args(["query", "HKCU\\Environment", "/v", "Path"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // Output shape: `    Path    REG_EXPAND_SZ    C:\...;D:\...`
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() != Some("Path") {
            continue;
        }
        if let Some(value) = parts.last() {
            return expand_windows_env(value)
                .map(|path| std::env::split_paths(&path).collect())
                .unwrap_or_default();
        }
    }
    Vec::new()
}

#[cfg(windows)]
fn expand_windows_env(value: &str) -> Option<std::ffi::OsString> {
    let mut expanded = String::new();
    for (index, segment) in value.split('%').enumerate() {
        if index % 2 == 0 {
            expanded.push_str(segment);
        } else {
            expanded.push_str(&std::env::var(segment).ok()?);
        }
    }
    Some(std::ffi::OsString::from(expanded))
}

/// True when `path` is a file this system would execute.
pub fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Resolve a bare program name (or an explicit path) to an executable.
pub fn resolve(binary: &str) -> Option<PathBuf> {
    let candidate = Path::new(binary);
    if candidate.is_absolute() || candidate.components().count() > 1 {
        return is_executable(candidate).then(|| candidate.to_path_buf());
    }

    let process_dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_default();
    if let Some(found) = resolve_in(binary, &process_dirs) {
        return Some(found);
    }
    resolve_in(binary, extra_dirs())
}

/// Same as [`resolve`], restricted to `dirs` (used by tests and probes with
/// an explicit search path).
pub fn resolve_in(binary: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in dirs {
        let candidate = dir.join(binary);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        for suffix in [".exe", ".cmd", ".bat"] {
            let candidate = dir.join(format!("{binary}{suffix}"));
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
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
        let deduped = dedupe(vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::new(),
        ]);
        assert_eq!(deduped, vec![PathBuf::from("/opt/homebrew/bin")]);
    }
}
