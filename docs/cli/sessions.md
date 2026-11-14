# OwnHub CLI: Sessions Management & Observability

`ownhub sessions <subcommand>` provides granular command-line management and real-time observability across all agent conversations stored by the eight supported harnesses.

---

## 1. Overview & Purpose

OwnHub unifies agent sessions across multiple AI coding harnesses:

- **Claude Code** (`claude`)
- **Antigravity** (`antigravity`)
- **OpenCode** (`opencode`)
- **Codex** (`codex`)
- **Copilot CLI** (`copilot`)
- **Kilo** (`kilo`)
- **Hermes** (`hermes`)
- **xAI Grok** (`grok`)

Session histories are stored natively by each harness in SQLite databases, JSONL files, or directory trees. The Sessions subsystem in OwnHub:

1. **Scans Read-Only:** Queries stores without modifying harness schemas or interrupting running agents.
2. **Maintains Policy & Pins:** Keeps operator pins and retention rules safely in `~/.ownhub/sessions_policy.json` (sidecar architecture).
3. **Provides Unified Observability:** Parses transcripts, tracks agent liveness, and extracts cross-agent handoff contexts.
4. **Enforces Safe Retention:** Implements dry-run purge estimation, active session locking guards, and vacuum page reclamation.

---

## 2. Current State vs. New Granular Design

### Current State

In the current codebase (`src-tauri/src/cli/native/sessions.rs`), only two subcommands exist:

- `ownhub sessions list [--harness <id>] [--search <term>] [--pinned-only] [--json]`
- `ownhub sessions purge [--harness <id>] [--older-than-days <n>] [--session-id <id>] [--yes] [--json]`

Other session capabilities exist in `crates/ownhub-sessions` and the `ownhub-session-relay` MCP server, but are not exposed through the native CLI.

### New Granular Design

The expanded CLI elevates `ownhub sessions` into a complete operator control plane:

- **Discovery & Querying:** `list`, `search`, `inspect`
- **Protection & Retention:** `pin`, `unpin`, `purge`, `policy`
- **Real-Time Monitoring:** `tail`, `watch`
- **Cross-Agent Collaboration:** `handoff`, `export`
- **Surgical Maintenance:** `delete`

All subcommands support human-readable formatted terminal output by default and machine-parsable `--json` output for automation and script integration.

---

## 3. Command Syntax & Subcommands

```bash
ownhub sessions <subcommand> [arguments] [options]
```

### 3.1 `list`

Lists sessions across one or all installed harnesses.

```bash
ownhub sessions list [options]
```

**Options:**

- `--harness <id>`: Filter by harness (e.g. `claude`, `antigravity`, `codex`).
- `--search <term>`: Case-insensitive substring match on session title or project path.
- `--pinned-only`: Display only pinned (protected) sessions.
- `--active-only`: Display only sessions currently active or held open by a running agent.
- `--limit <n>`: Limit output to `n` newest sessions.
- `--offset <n>`: Skip `n` sessions (for pagination).
- `--json`: Output as JSON array of `UnifiedSession`.

**Example:**

```bash
ownhub sessions list --harness claude --limit 10
ownhub sessions list --pinned-only --json
```

---

### 3.2 `inspect`

Displays detailed metadata and recent transcript turns of a specific session.

```bash
ownhub sessions inspect [<session-id>] [options]
```

**Options:**

- `<session-id>`: Unique session ID or slug. Defaults to `latest` if omitted.
- `--session-id <id>`: Alternative flag syntax for `<session-id>`.
- `--harness <id>`: Canonical harness ID. If omitted, the harness is inferred or auto-detected across stores.
- `--max-turns <n>`: Maximum number of conversation turns to display (default: 10, max: 1000).
- `--compact`: Output a concise one-line-per-turn view.
- `--json`: Output full session metadata and turn records as JSON.

**Example:**

```bash
ownhub sessions inspect latest --harness antigravity
ownhub sessions inspect c9e31d4b-7033-4f9e-a812 --max-turns 20 --compact
```

---

### 3.3 `pin`

Pins a session to protect it from automated or accidental purge.

```bash
ownhub sessions pin <session-id> [options]
```

**Options:**

- `<session-id>`: Target session ID.
- `--harness <id>`: Target harness ID (required if session ID is ambiguous across harnesses).
- `--label <text>`: Optional human-readable description or tag for the pin.
- `--json`: Output updated pin status as JSON.

**Example:**

```bash
ownhub sessions pin session-2026-11-05-01 --harness claude --label "Release v1.2 audit"
```

---

### 3.4 `unpin`

Removes the pin protection from a session, allowing standard retention rules to apply.

```bash
ownhub sessions unpin <session-id> [options]
```

**Options:**

- `<session-id>`: Target session ID.
- `--harness <id>`: Target harness ID.
- `--json`: Output unpin confirmation as JSON.

**Example:**

```bash
ownhub sessions unpin session-2026-11-05-01 --harness claude
```

---

### 3.5 `tail` / `watch`

Monitors active agent turns and events in real time or inspects the tail of a session.

```bash
ownhub sessions tail [<session-id>] [options]
ownhub sessions watch [options]
```

**Options:**

- `<session-id>`: Target session ID (defaults to newest active session).
- `--harness <id>`: Filter or target a specific harness.
- `--limit <n>`: Number of trailing turns to fetch (default: 5).
- `--follow`, `-f`: Keep listening for newly appended transcript lines (poll interval: 1s).
- `--json`: Output events as JSON lines.

**Example:**

```bash
ownhub sessions tail --harness antigravity --limit 5
ownhub sessions watch --follow
```

---

### 3.6 `handoff`

Generates a structured executive brief for handing over context from one agent/harness to another.

```bash
ownhub sessions handoff [<session-id>] [options]
```

**Options:**

- `<session-id>`: Source session ID (defaults to `latest`).
- `--source-harness <id>` / `--harness <id>`: Source harness ID.
- `--target-harness <id>`: Optional target harness for customized formatting.
- `--json`: Output handoff object `{ goal, filesTouched, blockers, nextSteps, transcriptSnippet }`.

**Example:**

```bash
ownhub sessions handoff latest --source-harness claude
ownhub sessions handoff latest --harness antigravity --target-harness codex
```

---

### 3.7 `search`

Searches across conversation histories, project workspaces, and message content.

```bash
ownhub sessions search <query> [options]
```

**Options:**

- `<query>`: Search string.
- `--query <text>`: Alternative flag syntax.
- `--harness <id>`: Restrict search to a specific harness.
- `--content`: Full-text search across transcript messages (by default searches titles and paths).
- `--limit <n>`: Maximum results to return (default: 15).
- `--json`: Output matching sessions and turn snippets as JSON.

**Example:**

```bash
ownhub sessions search "database migration" --content
ownhub sessions search "OwnHub" --harness claude --limit 5
```

---

### 3.8 `delete`

Surgically deletes an individual session across all files and SQLite stores of the harness.

```bash
ownhub sessions delete <session-id> [options]
```

**Options:**

- `<session-id>`: Session ID to delete.
- `--harness <id>`: Harness owning the session (required).
- `--yes`, `-y`: Confirm deletion without interactive prompt.
- `--force`: Delete even if the session is pinned (unpins before removal).
- `--json`: Output deletion summary `{ deleted: true, reclaimedBytes: u64 }`.

**Safety Guards:**

- Refuses to delete active sessions currently locked by a running agent.
- Refuses to delete pinned sessions unless `--force` is provided.

**Example:**

```bash
ownhub sessions delete sess_abc123 --harness codex --yes
```

---

### 3.9 `export`

Exports a session's metadata and transcript into a Markdown or JSON file.

```bash
ownhub sessions export <session-id> [options]
```

**Options:**

- `<session-id>`: Session ID to export.
- `--harness <id>`: Harness owning the session.
- `--format <markdown|json>`: Output format (default: `markdown`).
- `--output <path>`: Write to destination file instead of standard output.
- `--json`: Wrap command execution status in JSON.

**Example:**

```bash
ownhub sessions export latest --harness claude --format markdown --output ./session-export.md
ownhub sessions export sess_abc123 --harness antigravity --format json
```

---

### 3.10 `purge`

Cleans up expired sessions based on age and harness filters, with safe dry-run estimation.

```bash
ownhub sessions purge [options]
```

**Options:**

- `--harness <id>`: Limit purge to a specific harness.
- `--older-than-days <n>`: Purge sessions older than `n` days.
- `--session-id <id>`: Explicitly target specific session IDs (repeatable).
- `--yes`: Execute deletions. Without `--yes`, a dry-run estimate is performed.
- `--json`: Output report or estimate as JSON.

**Example:**

```bash
# Dry run
ownhub sessions purge --older-than-days 30
# Execution
ownhub sessions purge --older-than-days 30 --harness claude --yes
```

---

### 3.11 `policy`

Inspects or configures the retention policy in `~/.ownhub/sessions_policy.json`.

```bash
ownhub sessions policy show [options]
ownhub sessions policy set [options]
```

**`policy set` Options:**

- `--default-ttl <days>`: Global session time-to-live in days.
- `--auto-purge <on|off>`: Enable or disable background auto-purge scheduler.
- `--interval <hours>`: Scheduler execution interval in hours.
- `--override <harness=days>`: Set custom TTL for a specific harness (e.g. `codex=7`).
- `--json`: Output updated policy as JSON.

**Example:**

```bash
ownhub sessions policy show
ownhub sessions policy set --default-ttl 15 --auto-purge on --interval 24 --override codex=7
```

---

## 4. Architecture & Implementation Guidelines

### 4.1 Anti-Monolith De-composition

Per project architecture rules, files must stay strictly under **300-400 lines**.
The sessions CLI must be structured modularly under `src-tauri/src/cli/native/sessions/`:

```
src-tauri/src/cli/native/sessions/
├── mod.rs          # Subcommand dispatch, common error formatting
├── list.rs         # 'list' subcommand implementation
├── inspect.rs      # 'inspect' subcommand and turn rendering
├── pins.rs         # 'pin' and 'unpin' subcommands
├── watch.rs        # 'tail' and 'watch' subcommands
├── handoff.rs      # 'handoff' generation
├── search.rs       # 'search' subcommand across histories
├── export.rs       # 'export' markdown & json rendering
├── delete.rs       # Surgical 'delete' implementation
├── purge.rs        # Purge estimation and execution logic
└── policy.rs       # Retention policy inspect and update
```

### 4.2 Shared Backend Crates

- Rely on `crates/ownhub-sessions` (`adapters`, `policy`, `purge`, `types`).
- Respect zero-panic rules (`#![deny(clippy::unwrap_used)]`).
- All subcommands must return `Result<String, CliError>`.
