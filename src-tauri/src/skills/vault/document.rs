//! Frontmatter parsing and the stable category rules for a skill folder.

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub(in crate::skills) struct Frontmatter {
    pub(in crate::skills) name: Option<String>,
    pub(in crate::skills) description: Option<String>,
}

/// Split a `---` delimited YAML frontmatter block from the markdown body.
pub(in crate::skills) fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
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

pub(in crate::skills) fn normalize_description(raw: &str) -> String {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&collapsed, 320)
}

pub(in crate::skills) fn first_paragraph(body: &str) -> String {
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
pub(in crate::skills) fn estimate_tokens(content: &str) -> usize {
    (content.chars().count() as f64 / 3.8).round() as usize
}

/// Map a skill folder onto a stable, human-facing category. The identifier and
/// frontmatter name win; the description is only a fallback signal.
pub(in crate::skills) fn categorize(id: &str, name: &str, description: &str) -> &'static str {
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
