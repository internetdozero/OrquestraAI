//! The master `AGENTS.md`: the single instruction file every harness links
//! to, created from a template on first use and replaced atomically.

use std::fs;

use crate::paths::{create_dir, open_paths, unique_tmp_suffix, OrquestraPaths, AGENTS_FILE};

pub const DEFAULT_AGENTS: &str = r#"# Global Agent Instructions

> Managed by OrquestraAI. This master file is the single source of truth for every linked harness.

## Operating Principles

- Prefer retrieval over guessing; verify against the repository before acting.
- Make the smallest change that solves the problem, then prove it works.
- Never overwrite user data or discard uncommitted work without permission.
- Report failing checks honestly; never present unverified work as complete.

## Workflow

1. Read the relevant context before editing.
2. State the plan for multi-step work.
3. Validate with the project's own build, test, and typecheck commands.
"#;

/// Read the master instructions, creating a clean template on first use.
pub fn read_master_agents_md() -> Result<String, String> {
    let paths = open_paths()?;
    read_agents(&paths)
}

/// Atomically replace the master instructions file.
pub fn write_master_agents_md(content: &str) -> Result<(), String> {
    let paths = open_paths()?;
    write_agents(&paths, content)
}

pub fn read_agents(paths: &OrquestraPaths) -> Result<String, String> {
    let path = paths.agents_path();

    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            write_agents(paths, DEFAULT_AGENTS)?;
        }
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => {
            return Err(format!(
                "refusing to read {}: not a regular file",
                path.display()
            ));
        }
        Err(error) => {
            return Err(format!("failed to read {}: {error}", path.display()));
        }
    }

    fs::read_to_string(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

pub fn write_agents(paths: &OrquestraPaths, content: &str) -> Result<(), String> {
    create_dir(&paths.root)?;
    let path = paths.agents_path();
    let temporary = paths
        .root
        .join(format!(".{AGENTS_FILE}.tmp-{}", unique_tmp_suffix()));

    fs::write(&temporary, content).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to write {}: {error}", temporary.display())
    })?;

    fs::rename(&temporary, &path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to replace {}: {error}", path.display())
    })
}
