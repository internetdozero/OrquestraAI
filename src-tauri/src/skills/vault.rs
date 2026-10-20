//! Reading and indexing the master vault.
//!
//! Every skill is a folder holding a `SKILL.md`; the frontmatter names it
//! and describes it, and the body supplies a fallback description. The
//! projection state is layered on top by [`super::projection`].

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use std::collections::HashSet;

use super::projection::is_projected;
use super::security::scan_skill_security;
use super::vault_cache::{cached_content, prune_content_cache};
use super::{SkillItem, GLOBAL_SCOPE};
use crate::paths::OrquestraPaths;

/// Folder name expected at the root of every skill in the vault.
pub(crate) const SKILL_FILE: &str = "SKILL.md";

pub(super) fn list_from(paths: &OrquestraPaths, scope: &str) -> Result<Vec<SkillItem>, String> {
    let global_dir = paths.global_dir();
    let scope_dir = paths.projection_dir(scope);
    let is_global = scope == GLOBAL_SCOPE;
    let mut skills = Vec::new();
    let mut seen = HashSet::new();

    for (id, directory) in skill_directories(&paths.vault)? {
        // File bytes and YAML parse come from the vault cache (validated by
        // SKILL.md mtime+size); projection state and the security walk stay
        // live, because neither may ever serve a stale answer.
        let skill_file = directory.join(SKILL_FILE);
        let Some(cached) = cached_content(&directory, &id, &skill_file) else {
            continue;
        };
        seen.insert(directory.clone());

        let global_enabled = is_projected(&global_dir, &paths.vault, &id);
        let inherited_from_global = !is_global && global_enabled;
        let harness_enabled = if is_global {
            global_enabled
        } else {
            global_enabled || is_projected(&scope_dir, &paths.vault, &id)
        };

        let (has_scripts, script_files) = scan_skill_security(&directory);

        skills.push(SkillItem {
            id,
            name: cached.name,
            description: cached.description,
            category: cached.category,
            tokens: cached.tokens,
            global_enabled,
            harness_enabled,
            inherited_from_global,
            official: cached.official,
            has_scripts,
            script_files,
        });
    }

    prune_content_cache(&seen);
    skills.sort_by_key(|skill| skill.name.to_lowercase());
    Ok(skills)
}

/// Read the raw `SKILL.md` of one vault skill, for the in-app editor.
pub(super) fn read_skill_content(paths: &OrquestraPaths, id: &str) -> Result<String, String> {
    let directory = skill_directory(paths, id)?;
    let file = directory.join(SKILL_FILE);
    fs::read_to_string(&file)
        .map_err(|error| format!("failed to read {}: {error}", file.display()))
}

/// Replace a skill's `SKILL.md` atomically (tmp file + rename on the same
/// filesystem). Blank content is refused: an empty definition would silently
/// unlist the skill on the next scan.
pub(super) fn save_skill_content(
    paths: &OrquestraPaths,
    id: &str,
    content: &str,
) -> Result<(), String> {
    if content.trim().is_empty() {
        return Err("skill content must not be empty".to_string());
    }
    let directory = skill_directory(paths, id)?;
    let target = directory.join(SKILL_FILE);
    let temporary = directory.join(format!(".{SKILL_FILE}.tmp-{}", std::process::id()));

    fs::write(&temporary, content).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to write {}: {error}", temporary.display())
    })?;

    fs::rename(&temporary, &target).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("failed to replace {}: {error}", target.display())
    })
}

/// Remove a skill from the vault: unlink every projection, then delete the
/// folder. A vault entry that is not a real directory (symlink, file) is
/// refused so a malicious or broken entry can never escape the vault.
pub(super) fn delete_skill(paths: &OrquestraPaths, id: &str) -> Result<(), String> {
    let directory = skill_directory(paths, id)?;
    let kind = fs::symlink_metadata(&directory)
        .map_err(|error| format!("failed to inspect {}: {error}", directory.display()))?;
    if !kind.file_type().is_dir() {
        return Err(format!(
            "refusing to delete {}: not a real directory",
            directory.display()
        ));
    }

    super::projection::set_global(paths, id, false)?;
    fs::remove_dir_all(&directory)
        .map_err(|error| format!("failed to remove {}: {error}", directory.display()))
}

pub(super) fn skill_directory(paths: &OrquestraPaths, id: &str) -> Result<PathBuf, String> {
    validate_id(id)?;
    let directory = paths.vault.join(id);
    let metadata = fs::symlink_metadata(&directory)
        .map_err(|_| format!("unknown skill: {id}"))?;
    if !metadata.file_type().is_dir() {
        return Err(format!(
            "refusing to use {id}: vault entry is not a real directory"
        ));
    }
    Ok(directory)
}

pub(super) fn skill_directories(vault: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let entries = fs::read_dir(vault)
        .map_err(|error| format!("failed to read vault {}: {error}", vault.display()))?;

    let mut directories = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        // `is_dir` follows symlinks. A vault entry that points outside would
        // be listed and then projected into every harness.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        if !path.join(SKILL_FILE).is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || validate_id(&name).is_err() {
            continue;
        }

        directories.push((name, path));
    }

    directories.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(directories)
}

/// Guard against path traversal and hidden/unusual folder names.
pub(super) fn validate_id(id: &str) -> Result<(), String> {
    let invalid = id.is_empty()
        || id == "."
        || id == ".."
        || id.starts_with('.')
        || id.contains('/')
        || id.contains('\\')
        || id.contains('\0');

    if invalid {
        Err(format!("invalid skill id: {id:?}"))
    } else {
        Ok(())
    }
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct Frontmatter {
    pub(super) name: Option<String>,
    pub(super) description: Option<String>,
}

/// Split a `---` delimited YAML frontmatter block from the markdown body.
pub(super) fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let text = content.strip_prefix('\u{feff}').unwrap_or(content);

    let first_line = match text.split_inclusive('\n').next() {
        Some(line) if line.trim() == "---" => line,
        _ => return (None, text),
    };

    let yaml_start = first_line.len();
    let mut offset = yaml_start;

    for line in text.split_inclusive('\n').skip(1) {
        if line.trim() == "---" || line.trim() == "..." {
            let yaml = &text[yaml_start..offset];
            let body = &text[offset + line.len()..];
            return (Some(yaml), body);
        }
        offset += line.len();
    }

    (None, text)
}

pub(super) fn normalize_description(raw: &str) -> String {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&collapsed, 320)
}

pub(super) fn first_paragraph(body: &str) -> String {
    let paragraph = body
        .lines()
        .map(str::trim)
        .skip_while(|line| line.is_empty() || line.starts_with('#'))
        .take_while(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    truncate(
        &paragraph
            .replace(['`', '*', '_'], "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        320,
    )
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut shortened = value.chars().take(max_chars).collect::<String>();
    shortened.push('…');
    shortened
}

/// Heuristic prompt weight: the PRD models roughly 3.8 characters per token.
pub(super) fn estimate_tokens(content: &str) -> usize {
    (content.chars().count() as f64 / 3.8).round() as usize
}

/// Map a skill folder onto a stable, human-facing category. The identifier and
/// frontmatter name win; the description is only a fallback signal.
pub(super) fn categorize(id: &str, name: &str, description: &str) -> &'static str {
    const RULES: &[(&str, &[&str])] = &[
        (
            "Security",
            &[
                "security",
                "pentest",
                "penetration",
                "exploit",
                "vulnerab",
                "reverse",
                "decompil",
                "malware",
                "forensic",
                "threat",
                "osint",
                "burp",
                "fuzz",
                "hardening",
            ],
        ),
        (
            "Database",
            &[
                "database",
                "sql",
                "postgres",
                "sqlite",
                "mysql",
                "mongo",
                "redis",
                "airtable",
                "supabase",
                "databricks",
                "duckdb",
            ],
        ),
        (
            "Cloud",
            &[
                "cloud",
                "aws",
                "gcp",
                "azure",
                "cloudflare",
                "wrangler",
                "workers",
                "vercel",
                "netlify",
                "render",
                "serverless",
                "sandbox",
                "durable",
                "s3",
                "lambda",
            ],
        ),
        (
            "Infra",
            &[
                "docker",
                "container",
                "kubernetes",
                "k8s",
                "helm",
                "infra",
                "devops",
                "deploy",
                "terraform",
                "ansible",
                "nginx",
                "linux",
                "systemd",
                "ci-cd",
                "github-actions",
                "shell",
                "git-workflow",
            ],
        ),
        (
            "Frontend",
            &[
                "frontend",
                "front-end",
                "ui-",
                "-ui",
                "design",
                "css",
                "tailwind",
                "react",
                "vue",
                "svelte",
                "nextjs",
                "figma",
                "accessib",
                "a11y",
                "p5js",
                "web-",
                "webperf",
                "web-perf",
                "canvas",
            ],
        ),
        (
            "Backend",
            &[
                "backend",
                "api-and",
                "api-",
                "-api",
                "graphql",
                "grpc",
                "aspnet",
                "django",
                "flask",
                "express",
                "node-",
                "python-",
                "jupyter",
                "sdk",
            ],
        ),
        (
            "AI & Agents",
            &[
                "agent",
                "llm",
                "openai",
                "anthropic",
                "claude",
                "codex",
                "gemini",
                "prompt",
                "embedding",
                "subagent",
                "orchestr",
                "multiagent",
                "generative",
                "imagegen",
                "copilot",
                "harness",
            ],
        ),
        (
            "Testing",
            &[
                "test", "qa-", "-qa", "playwright", "cypress", "jest", "vitest", "debug",
                "review", "lint", "audit",
            ],
        ),
        (
            "Docs",
            &[
                "docs",
                "doc-",
                "documentation",
                "readme",
                "markdown",
                "docx",
                "pdf",
                "xlsx",
                "citation",
                "writing",
                "humanizer",
                "content",
                "wiki",
            ],
        ),
        (
            "Productivity",
            &[
                "notion",
                "linear",
                "github",
                "git-",
                "email",
                "inbox",
                "calendar",
                "meeting",
                "notes",
                "obsidian",
                "apple-",
                "imessage",
                "google-workspace",
                "teams",
                "maps",
                "planning",
                "review",
                "monitor",
                "workflow",
                "task",
                "shopping",
                "price",
            ],
        ),
        (
            "Media",
            &[
                "video",
                "image",
                "audio",
                "music",
                "song",
                "speech",
                "transcribe",
                "manim",
                "ascii",
                "gif",
                "screenshot",
                "photo",
                "infographic",
                "virtual-piano",
            ],
        ),
        (
            "Communication",
            &["caveman", "writing", "interview", "humanizer", "chat"],
        ),
    ];

    let identity = format!("{id} {name}").to_lowercase();
    for (category, keywords) in RULES {
        if keywords.iter().any(|keyword| identity.contains(keyword)) {
            return category;
        }
    }

    let haystack = description.to_lowercase();
    for (category, keywords) in RULES {
        if keywords.iter().any(|keyword| haystack.contains(keyword)) {
            return category;
        }
    }

    "General"
}
