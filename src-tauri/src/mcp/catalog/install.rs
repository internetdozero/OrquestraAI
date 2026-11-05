//! Install curated MCP packages into Orquestra's private data root.

use super::super::McpServer;
use crate::official::official_store;
use crate::paths::OrquestraPaths;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn ensure_official_server(
    server: &mut McpServer,
    paths: &OrquestraPaths,
) -> Result<(), String> {
    if !crate::official::is_official_mcp(&server.id) {
        return Ok(());
    }
    let Some(entry) = official_store()
        .mcps
        .into_iter()
        .find(|entry| entry.id == server.id)
    else {
        return Ok(());
    };
    if let Some(executable) = find_command(&server.command) {
        server.command = executable.to_string_lossy().into_owned();
        return Ok(());
    }

    let mut repositories = Vec::new();
    if let Some(url) = entry.repo_url {
        repositories.push(url);
    }
    if let Some(url) = entry.fallback_repo_url {
        if !repositories.contains(&url) {
            repositories.push(url);
        }
    }
    if repositories.is_empty() {
        return Err(format!("{} has no install source", entry.name));
    }

    let base = paths.root.join("mcp-servers").join(&server.id);
    create_private_dir(paths, &base)?;
    let mut failures = Vec::new();
    for repository in repositories {
        let install_dir = allocate_install_dir(&base)?;
        match install_repository(&repository, &entry.command, &install_dir) {
            Ok(executable) => {
                server.command = executable.to_string_lossy().into_owned();
                return Ok(());
            }
            Err(error) => {
                failures.push(error);
                let _ = fs::remove_dir_all(&install_dir);
            }
        }
    }

    Err(format!(
        "could not install {}: {}",
        entry.name,
        failures.join("; ")
    ))
}

fn install_repository(
    repository: &str,
    command: &str,
    install_dir: &Path,
) -> Result<PathBuf, String> {
    let (clone_url, anchor) = crate::skills::split_git_url(repository);
    let source_root = install_dir.join("repo");
    crate::skills::clone_shallow(&clone_url, &source_root)?;
    let package = anchor
        .as_deref()
        .map(|anchor| source_root.join(anchor))
        .unwrap_or_else(|| source_root.clone());
    if !is_real_directory(&package) || !is_real_file(&package.join("pyproject.toml")) {
        return Err(format!(
            "{} does not contain an installable Python package at its configured path",
            repository
        ));
    }

    let virtual_env = install_dir.join(".venv");
    let python = python_command();
    run(
        Command::new(python).arg("-m").arg("venv").arg(&virtual_env),
        "create the MCP's private Python environment",
    )?;

    let env_python = venv_python(&virtual_env);
    run(
        Command::new(&env_python)
            .args([
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--no-input",
                "--retries",
                "1",
                "--timeout",
                "30",
            ])
            .arg(&package),
        "install the MCP package and its dependencies",
    )?;

    let executable = venv_executable(&virtual_env, command);
    if !is_real_file(&executable) {
        return Err(format!(
            "installation completed, but the package did not create its `{command}` executable"
        ));
    }
    Ok(executable)
}

fn create_private_dir(paths: &OrquestraPaths, path: &Path) -> Result<(), String> {
    let relative = path
        .strip_prefix(&paths.root)
        .map_err(|_| format!("refusing MCP install outside {}", paths.root.display()))?;
    let mut current = paths.root.clone();
    for component in relative.components() {
        current.push(component);
        if is_symlink(&current) {
            return Err(format!(
                "refusing to install MCP through symlinked path {}",
                current.display()
            ));
        }
    }
    fs::create_dir_all(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))
}

fn allocate_install_dir(base: &Path) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before the Unix epoch: {error}"))?
        .as_nanos();
    for attempt in 0..100 {
        let candidate = base.join(format!("install-{stamp}-{attempt}"));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("failed to create {}: {error}", candidate.display())),
        }
    }
    Err("could not allocate a private MCP installation directory".to_string())
}

fn run(command: &mut Command, action: &str) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("could not {action}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if detail.is_empty() {
        format!("could not {action} (process exited with {})", output.status)
    } else {
        format!("could not {action}: {detail}")
    })
}

pub(super) fn find_command(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.components().count() > 1 || path.is_absolute() {
        return is_executable(path).then(|| path.to_path_buf());
    }
    let search = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&search) {
        let candidate = directory.join(command);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        for suffix in [".exe", ".cmd", ".bat"] {
            let candidate = directory.join(format!("{command}{suffix}"));
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn python_command() -> &'static str {
    if cfg!(windows) {
        "python"
    } else {
        "python3"
    }
}

fn venv_python(virtual_env: &Path) -> PathBuf {
    if cfg!(windows) {
        virtual_env.join("Scripts/python.exe")
    } else {
        virtual_env.join("bin/python")
    }
}

fn venv_executable(virtual_env: &Path, command: &str) -> PathBuf {
    if cfg!(windows) {
        virtual_env.join(format!("Scripts/{command}.exe"))
    } else {
        virtual_env.join(format!("bin/{command}"))
    }
}

fn is_real_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_installed_commands_to_executable_paths() {
        #[cfg(unix)]
        assert!(find_command("/bin/echo").is_some());
        assert!(find_command("orquestra-test-command-that-does-not-exist").is_none());
    }
}
