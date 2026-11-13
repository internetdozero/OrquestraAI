# Orquestra CLI — Skills Management (`orquestra skills`)

Comprehensive specification and operational guide for the granular Skills CLI in OrquestraAI.

---

## 1. Overview & Architecture

In OrquestraAI, **Skills** are autonomous capability packages provided to AI agents across the supported harnesses (Antigravity/AGY, Claude Code, Codex CLI, Copilot CLI, OpenCode, Kilo Code, Hermes Agent and Grok).

### Domain Model & Storage Layout

- **Single Source of Truth (Vault):**
  - Path: `~/.orquestra/skills-library/<skill-id>/` (resolved via `OrquestraPaths::vault`).
  - Marker file: Every skill directory must contain a valid `SKILL.md` (constant `SKILL_FILE = "SKILL.md"`).
  - Metadata: Parsed from YAML frontmatter (`name`, `description`, `category`) and body markdown.
- **Scope-based Projections:**
  - Path: `~/.orquestra/active/<scope>/<skill-id>` (resolved via `OrquestraPaths::projection_dir`).
  - Active skills exist as directory links targeting the vault directory: a
    symlink on Linux and macOS, an NTFS junction on Windows (creating a
    Windows symlink needs elevation or Developer Mode). See
    `docs/decisions/windows-projections.md`.
  - **Global Scope (`global`):** Universal baseline. When a skill is enabled in `global`, it is automatically projected into `active/global/` and propagated to all harness active directories (`active/<harness_id>/`). A globally enabled skill is inherited across all harnesses and cannot be disabled in an isolated harness without first disabling it in `global`.
  - **Harness Scopes (e.g. `claude`, `gemini`, `codex`, `copilot`, `cursor`):** Isolated projections. Enabling a skill here symlinks it only into that specific harness scope without touching others.
- **Static Security Auditing:**
  - Automated scanning (`scan_skill_security`) checks skill contents for executable payloads:
    - Files with extensions: `.sh`, `.bash`, `.py`, `.js`, `.mjs`.
    - Any files inside `scripts/` or `bin/` directories.
  - Bounded walk (max depth 4, max 500 files, skipping `.git`, `node_modules`, `target`) flags `has_scripts` and populates `script_files` for operator visibility.
- **Master Instructions:**
  - Path: `~/.orquestra/AGENTS.md`. Linked into harnesses during fusion.

---

## 2. Current State vs. Granular CLI Design

### What Exists Today

Prior to this specification, skill management was primarily exposed via:

1. **Tauri IPC Commands:** `list_skills`, `set_skill_state`, `batch_set_all`, `read_skill_content`, `save_skill_content`, `delete_skill`, `import_skill`, `import_skill_from_git`, `harness_summary`.
2. **MCP Tools (`orquestra-core`):** `orquestra_list_skills`, `orquestra_enable_skill`, `orquestra_disable_skill`, `orquestra_create_skill`, `orquestra_delete_skill`.
3. **Native CLI (`dispatch_native_cli`):** Only implemented roots for `ideas`, `sessions`, `bridge`, and `vault`. Headless shell scripts or automated CI had **no** direct `orquestra skills` CLI command and had to rely on manual directory inspections or GUI operations.

### The Granular CLI Design

The `orquestra skills` command suite brings full native CLI parity to the terminal, scripting pipelines, and autonomous builder agents.

Key capabilities:

- **Zero GUI dependency:** Operates completely headlessly against `~/.orquestra`.
- **Dual Output Modes:** Clean, aligned human-readable tables/summaries by default, and strict, typed `--json` output for automated tooling.
- **Deep Validation & Security Scans:** Inspects YAML frontmatter, detects path traversal attempts, and checks executable scripts before or after installation.
- **Monorepo Git & Local Imports:** Clone-and-import with subpath discovery or explicit anchors (`repo.git#subpath`).
- **Scaffolding & Authoring:** Scaffold compliant skill templates with standard directories (`scripts/`, `references/`) and edit contents programmatically.

---

## 3. Command Matrix & Syntax

Root command:

```bash
orquestra skills <subcommand> [arguments] [options]
```

### General Options

- `--json`: Outputs machine-readable JSON to `stdout`. All diagnostics/errors are routed to `stderr` with non-zero exit codes.
- `-h`, `--help`: Prints command or subcommand help.

### Summary Table

| Subcommand            | Arguments           | Options                                                                                           | Description                                                                           |
| --------------------- | ------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `list`                | —                   | `--scope <id>`, `--filter <str>`, `--official`, `--has-scripts`, `--json`                         | List vault skills with projection status.                                             |
| `info` / `inspect`    | `<id>`              | `--scope <id>`, `--json`                                                                          | Inspect detailed skill metadata, security walk, and projection matrix.                |
| `enable`              | `<id>`              | `--scope <id>`, `--json`                                                                          | Enable (project) a skill into global baseline or a specific harness.                  |
| `disable`             | `<id>`              | `--scope <id>`, `--force`, `--json`                                                               | Disable (un-project) a skill from a harness or global baseline.                       |
| `batch`               | `<enable\|disable>` | `--scope <id>`, `--json`                                                                          | Enable or disable all skills in a specified scope.                                    |
| `summary`             | —                   | `--json`                                                                                          | Display active/total skill counts across all scopes.                                  |
| `validate`            | `<path\|id>`        | `--json`                                                                                          | Validate a skill's directory structure, `SKILL.md` frontmatter, and security profile. |
| `create` / `scaffold` | `<id>`              | `--name <str>`, `--description <str>`, `--category <cat>`, `--with-scripts`, `--enable`, `--json` | Scaffold a new compliant skill in the vault.                                          |
| `import`              | `<path\|git_url>`   | `--subpath <path>`, `--scope <id>`, `--enable`, `--json`                                          | Import a skill from a local folder or remote Git repository.                          |
| `export`              | `<id>`              | `--out <dir>`, `--tar`, `--json`                                                                  | Export a vault skill to a standalone directory or archive.                            |
| `content get`         | `<id>`              | —                                                                                                 | Print raw `SKILL.md` content to stdout.                                               |
| `content set`         | `<id>`              | `--file <path>`, `--json`                                                                         | Atomically replace `SKILL.md` from a file or stdin (`-`).                             |
| `delete`              | `<id>`              | `--force`, `-f`, `--json`                                                                         | Unlink projections and delete a skill folder permanently from the vault.              |

---

## 4. Subcommand Specifications & Practical Examples

### 4.1. `orquestra skills list`

Lists all skills discovered in the vault, evaluated for a target scope.

- **Options:**
  - `--scope <scope>`: Scope to evaluate against (`global` or harness ID like `gemini`, `claude`, `codex`). Defaults to `global`.
  - `--filter <text>`: Case-insensitive search matching name, description, or ID.
  - `--official`: Only show official tier skills.
  - `--has-scripts`: Only show skills containing executable helper scripts.
  - `--json`: Emit JSON array matching `Vec<SkillItem>`.

**Example:**

```bash
# Human readable list for global scope
orquestra skills list

# Output:
# ID                 NAME                   CATEGORY       SCOPE    SCRIPTS  OFFICIAL
# hardware-opt       Hardware Optimization  system         global   no       yes
# orquestra          Orquestra Ecosystem    orchestration  global   no       yes
# web-search         Web Search Tool        tools          global   yes      no

# JSON filtered by harness
orquestra skills list --scope claude --json
```

---

### 4.2. `orquestra skills info` / `orquestra skills inspect`

Inspects a single skill in depth, including token estimations, detailed frontmatter, detected script files, and projection status across all supported harnesses.

- **Arguments:** `<id>`: The unique identifier (folder name) of the skill.
- **Options:**
  - `--json`: Emit full JSON metadata object.

**Example:**

```bash
orquestra skills info orquestra-core

# Output:
# Skill: orquestra-core
# Name: Orquestra Core MCP Tools
# Category: orchestration
# Official: yes
# Tokens: ~1,240
# Path: /home/user/.orquestra/skills-library/orquestra-core
# Scripts Detected: 2 files
#   - scripts/bootstrap.sh
#   - scripts/sync.py
#
# Projection Matrix:
#   - global:   ENABLED
#   - claude:   ENABLED (inherited from global)
#   - gemini:   ENABLED (inherited from global)
#   - codex:    ENABLED (inherited from global)
```

---

### 4.3. `orquestra skills enable` & `orquestra skills disable`

Controls symlink projection for a skill.

- **Arguments:** `<id>`: Skill ID to toggle.
- **Options:**
  - `--scope <scope>`: Target scope (`global` or harness ID). Default: `global`.
  - `--force`: Required when disabling a skill at the harness level if inherited, or suppresses confirmation.
  - `--json`: Emits mutation status.

**Rules & Edge Cases:**

1. Enabling in `--scope global` creates symlinks in `active/global/` and in all harness folders.
2. Disabling in `--scope <harness>` when the skill is enabled in `global` returns an error:
   `"Skill '<id>' is inherited from Global; disable it in Global first or use --scope global"`.

**Examples:**

```bash
# Enable universally across all agents
orquestra skills enable hardware-optimization --scope global

# Enable only for Gemini
orquestra skills enable research-agent --scope gemini

# Disable in Gemini
orquestra skills disable research-agent --scope gemini
```

---

### 4.4. `orquestra skills batch`

Performs bulk projection changes for an entire scope.

- **Syntax:** `orquestra skills batch <enable|disable> [--scope <scope>] [--json]`
- **Examples:**

```bash
# Enable all vault skills for Codex harness
orquestra skills batch enable --scope codex

# Disable all harness-specific skills for Claude (inherited global skills remain active)
orquestra skills batch disable --scope claude
```

---

### 4.5. `orquestra skills summary`

Returns a fast overview of active vs total skills per harness and global baseline.

**Example:**

```bash
orquestra skills summary

# Output:
# SCOPE     ACTIVE  TOTAL
# global    12      24
# claude    15      24
# gemini    12      24
# codex     18      24
```

---

### 4.6. `orquestra skills validate`

Performs static analysis on a skill directory (in-vault or arbitrary filesystem path) to verify standard compliance before deployment or distribution.

- **Checks:**
  1. Directory exists and is a real directory (not broken symlink).
  2. ID safety: no path traversal (`..`), slashes, null bytes, or leading dots.
  3. `SKILL.md` existence and regular file metadata.
  4. Frontmatter parsing: valid YAML containing `name` and `description`.
  5. Security audit: lists helper scripts, checks file count cap (<500), and recursion depth (<4).
- **Exit codes:** `0` if valid, `1` if invalid.

**Example:**

```bash
orquestra skills validate ./my-new-skill --json
```

---

### 4.7. `orquestra skills create` / `orquestra skills scaffold`

Scaffolds a compliant skill directory in `~/.orquestra/skills-library/<id>/`.

- **Arguments:** `<id>`: Unique skill identifier (kebab-case alphanumeric).
- **Options:**
  - `--name <str>`: Human-readable display name (default: formatted `<id>`).
  - `--description <str>`: Brief skill description.
  - `--category <str>`: Category tag (default: `general`).
  - `--with-scripts`: Creates a `scripts/` directory with a sample script template.
  - `--enable`: Immediately enables globally after creation.
  - `--json`: Emits created `SkillItem`.

**Example:**

```bash
orquestra skills create custom-linter \
  --name "Custom Linter" \
  --description "Project-specific linting rules and autofix scripts." \
  --category "quality" \
  --with-scripts \
  --enable
```

---

### 4.8. `orquestra skills import`

Imports an external skill into the master vault.

- **Supports:**
  - Local folders: `orquestra skills import /path/to/skill-folder`
  - Git repositories: `orquestra skills import https://github.com/org/repo.git`
  - Monorepo subpaths: `orquestra skills import https://github.com/org/repo.git#skills/sub-skill`
- **Options:**
  - `--enable`: Automatically projects after import.
  - `--scope <scope>`: Scope to enable into if `--enable` is passed (default: `global`).
  - `--json`: Emits imported `SkillItem`.

**Example:**

```bash
orquestra skills import https://github.com/internetdozero/orquestra-extensions.git#skills/docker-tools --enable
```

---

### 4.9. `orquestra skills export`

Exports a skill from the vault to an external folder or tarball archive.

**Example:**

```bash
orquestra skills export hardware-optimization --out /tmp/exported-skills/ --tar
```

---

### 4.10. `orquestra skills content get` & `content set`

Read or atomically update the raw `SKILL.md` content.

**Examples:**

```bash
# Print raw SKILL.md to stdout
orquestra skills content get orquestra

# Replace content from a file
orquestra skills content set orquestra --file ./updated-SKILL.md

# Pipe content through stdin
cat new-skill.md | orquestra skills content set orquestra --file -
```

---

### 4.11. `orquestra skills delete`

Permanently unlinks a skill from all active projections and removes its directory from `~/.orquestra/skills-library/`.

- **Arguments:** `<id>`
- **Options:**
  - `-f`, `--force`: Skips interactive prompt in terminal. Mandatory in automated scripts.
  - `--json`: Emits deletion confirmation report.

**Example:**

```bash
orquestra skills delete obsolete-skill --force
```

---

## 5. JSON Schemas & Return Contracts

When `--json` is supplied, commands output strict JSON structures matching backend types.

### `SkillItem` Schema

```json
{
  "id": "hardware-optimization",
  "name": "Hardware Optimization",
  "description": "Hardware-aware execution tuning and CPU thermal preservation.",
  "category": "system",
  "tokens": 420,
  "official": true,
  "hasScripts": true,
  "scriptFiles": ["scripts/profile_cpu.sh"],
  "globalEnabled": true,
  "harnessEnabled": true,
  "inheritedFromGlobal": false
}
```

### `HarnessSummary` Schema

```json
[
  { "id": "global", "active": 8, "total": 24 },
  { "id": "claude", "active": 10, "total": 24 },
  { "id": "gemini", "active": 8, "total": 24 },
  { "id": "codex", "active": 12, "total": 24 }
]
```

### `ValidationReport` Schema

```json
{
  "valid": true,
  "id": "my-skill",
  "name": "My Skill",
  "description": "Description here",
  "category": "tools",
  "hasScripts": false,
  "scriptFiles": [],
  "errors": [],
  "warnings": []
}
```

---

## 6. Exit Codes

| Exit Code | Meaning              | Example Cause                                                                          |
| --------- | -------------------- | -------------------------------------------------------------------------------------- |
| `0`       | Success              | Operation completed normally; JSON or table emitted.                                   |
| `1`       | Operational Failure  | Skill ID not found, filesystem write error, git clone failure, path traversal attempt. |
| `2`       | Syntax / Usage Error | Missing required argument, unrecognized option, invalid integer/boolean format.        |

---

## 7. Implementation Blueprint for Builder AI

### Architecture Guidelines (Anti-Monolith & Modular Rust)

1. **Directory Structure:**
   Place the CLI implementation under `src-tauri/src/cli/native/skills/`:
   - `mod.rs`: Dispatching, argument routing, and top-level help (<150 lines).
   - `list.rs`: Handling of `list`, `summary`, and formatting (<250 lines).
   - `inspect.rs`: Handling of `info`/`inspect` and `validate` (<250 lines).
   - `mutate.rs`: Handling of `enable`, `disable`, `batch`, `delete` (<250 lines).
   - `manage.rs`: Handling of `create`, `import`, `export`, and `content` (<300 lines).
2. **Registration:**
   Register `skills` inside `src-tauri/src/cli/native.rs`:
   ```rust
   // in dispatch_native_cli:
   Some("skills") => Some(skills::run(&args[1..])),
   ```
3. **Re-use Existing Logic:**
   Directly leverage:
   - `crate::skills::list_skills`
   - `crate::skills::set_skill_state`
   - `crate::skills::batch_set_all`
   - `crate::skills::read_skill_content`
   - `crate::skills::save_skill_content`
   - `crate::skills::delete_skill`
   - `crate::skills::import_skill`
   - `crate::skills::import_skill_from_git`
   - `crate::skills::harness_summary`
   - `crate::skills::security::scan_skill_security`
4. **Testing Requirements:**
   Provide dedicated CLI unit and contract tests in `src-tauri/src/cli/native/skills/tests.rs` verifying:
   - Help / usage errors exit with code 2.
   - List and inspect render both human text and `--json`.
   - Traversal protection rejects `../` and malicious identifiers.
   - Batch and single enable/disable mutations accurately update filesystem projections.
