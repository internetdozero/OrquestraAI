# Orquestra CLI — Model Context Protocol (`mcp`)

The `orquestra mcp` subcommand family provides granular command-line lifecycle management, inspection, testing, and tool execution for Model Context Protocol (MCP) servers configured in OrquestraAI.

---

## 1. Overview & Architecture

OrquestraAI acts as a centralized **orchestration hub** for MCP servers across multiple AI coding harnesses (e.g., Claude Desktop, Cursor, Gemini Code Assist, Codex CLI, Windsurf, OpenCode).

### Key Architectural Pillars:

1. **Master Catalog Single Source of Truth (`~/.orquestra/mcps.json`):**
   - The master catalog stores all server definitions (command, args, env, headers, URLs, enabled targets).
   - Encrypted at rest using **AES-256-GCM** (via `orquestra_vault` / `at_rest` crate) and written with strict permissions (`0600`).
2. **Multi-Harness Projection (`deploy`):**
   - Whenever an MCP server is added, removed, or toggled, OrquestraAI translates the canonical representation into each target harness's native config dialect (e.g. `claude_desktop_config.json`, `gemini/settings.json`, `cursor/mcp.json`).
   - Foreign/unmanaged entries in those configs are preserved untouched.
3. **Vault Secret Interpolation:**
   - Environment variables and headers can reference blind vault credentials (`${VAULT:SERVICE_KEY}` or `.env` entries).
   - Plaintext tokens are masked during normal inspection and only decrypted at execution/runtime or upon explicit authenticated operator reveal.
4. **Official Hub & Extensions:**
   - Curated servers (`orquestra-idea-vault`, `orquestra-session-relay`, `orquestra-cloudflare`, `orquestra-core`, `orquestra-janitor`) are tagged as official and managed with automated installation fallbacks.

---

## 2. Current State vs. Granular CLI Design

| Capability              | Legacy CLI / Flags                           | New Granular CLI (`orquestra mcp <cmd>`)                                         |
| :---------------------- | :------------------------------------------- | :------------------------------------------------------------------------------- |
| **Catalog Listing**     | `--mcp-status` (raw JSON dump only)          | `orquestra mcp list [--enabled-only] [--harness <H>] [--official-only] [--json]` |
| **System Summary**      | Incomplete                                   | `orquestra mcp status [--json]`                                                  |
| **Server Inspection**   | None (manual JSON parsing)                   | `orquestra mcp inspect <id> [--reveal] [--json]`                                 |
| **Server Registration** | UI only                                      | `orquestra mcp add --id <id> --name <name> --command <cmd> [options] [--json]`   |
| **Server Deletion**     | UI only                                      | `orquestra mcp remove <id> [--json]`                                             |
| **Scope Toggling**      | `--mcp-set <id> <global\|harness> <on\|off>` | `orquestra mcp enable <id> [--harness <H>]` / `disable` / `set-state`            |
| **Harness Deploy**      | `--mcp-deploy`                               | `orquestra mcp deploy [--json]`                                                  |
| **Target Discovery**    | UI only                                      | `orquestra mcp targets [--json]`                                                 |
| **Connection Probe**    | None                                         | `orquestra mcp test-connection <id> [--timeout <ms>] [--json]`                   |
| **Tool Introspection**  | None                                         | `orquestra mcp tools-list <id> [--timeout <ms>] [--json]`                        |
| **Tool Invocation**     | None                                         | `orquestra mcp call-tool <id> <tool> [--args <json>] [--json]`                   |

---

## 3. Subcommand Reference

### 3.1 `orquestra mcp list`

Lists all MCP servers registered in the master catalog.

```bash
orquestra mcp list [OPTIONS]
```

**Options:**

- `--enabled-only`: Only display servers that are active globally or in at least one harness.
- `--global-only`: Only display servers enabled in global scope.
- `--harness <name>`: Filter servers enabled for a specific harness (e.g. `claude`, `cursor`, `gemini`).
- `--official-only`: Show only curated official extensions.
- `--search <term>`: Case-insensitive filter matching server ID, name, or description.
- `--json`: Output as structured JSON array.

**Human Output Example:**

```text
ID                      NAME             SCOPE               COMMAND        STATUS
orquestra-idea-vault    Idea Vault       global, gemini      idea-vault     active (official)
orquestra-session-relay Session Relay    global              session-relay  active (official)
github-context          GitHub MCP       cursor              npx            active
custom-postgres         Postgres DB      disabled            docker         inactive
```

---

### 3.2 `orquestra mcp status`

Provides a holistic diagnostic health status of the MCP subsystem.

```bash
orquestra mcp status [OPTIONS]
```

**Options:**

- `--json`: Output full health breakdown in JSON format.

**Human Output Example:**

```text
MCP Subsystem Status:
  Total Servers:      6 (2 official, 4 custom)
  Globally Active:    3
  Per-Harness Active: claude (4), cursor (3), gemini (3), codex (2)
  Harness Targets:    4 installed / 5 recognized
  Sync Status:        All configs in sync with master catalog
```

---

### 3.3 `orquestra mcp inspect <id>`

Inspects full configuration details of a specific MCP server. Alias: `info`.

```bash
orquestra mcp inspect <id> [OPTIONS]
```

**Options:**

- `--reveal`: Reveal plain secrets instead of masked strings (`***`). Requires operator session.
- `--json`: Output server definition in JSON.

**Human Output Example:**

```text
ID:          github-context
Name:        GitHub MCP
Description: GitHub issues and PR inspector
Official:    false
Command:     npx
Args:        -y @modelcontextprotocol/server-github
Environment:
  GITHUB_PERSONAL_ACCESS_TOKEN: ghp_••••••••••••••••••••
Scopes:
  Global:     false
  Harnesses:  cursor, claude
Status:       active (2 harnesses)
```

---

### 3.4 `orquestra mcp add`

Registers a new MCP server in the catalog and immediately projects it into targeted harness configs.

```bash
orquestra mcp add --id <id> --name <name> --command <cmd> [OPTIONS]
```

**Required Options:**

- `--id <id>`: Unique server identifier (alphanumeric, hyphens, underscores).
- `--name <name>`: Display label.
- `--command <cmd>`: Executable command (e.g., `npx`, `uvx`, `docker`, `/path/to/binary`).

**Optional Options:**

- `--args <arg1,arg2,...>`: Comma-separated arguments or repeated `--arg <val>`.
- `--env <KEY=VAL>`: Environment variable (can be specified multiple times).
- `--url <url>`: Endpoint URL for SSE/HTTP based MCP servers.
- `--header <KEY=VAL>`: HTTP Header for remote MCP servers.
- `--description <text>`: Brief functional description.
- `--global`: Enable globally across all installed harnesses.
- `--harness <name>`: Enable for a specific harness (can be repeated).
- `--json`: Output newly created server object in JSON.

**Example:**

```bash
orquestra mcp add \
  --id sqlite-db \
  --name "SQLite Database" \
  --command "uvx" \
  --args "mcp-server-sqlite,--db-path,/home/user/app.db" \
  --harness cursor \
  --description "Local development SQLite database access"
```

---

### 3.5 `orquestra mcp remove <id>`

Removes an MCP server from the catalog and purges its definition from all harness configuration files. Alias: `delete`.

```bash
orquestra mcp remove <id> [OPTIONS]
```

**Options:**

- `--json`: Output deletion confirmation as JSON.

**Human Output Example:**

```text
Removed MCP server 'sqlite-db' from catalog and purged from 2 harness configuration files.
```

---

### 3.6 `orquestra mcp enable` / `disable` / `set-state`

Controls the active scope of an MCP server.

```bash
# Convenient toggle shortcuts:
orquestra mcp enable <id> [--global] [--harness <name>] [--json]
orquestra mcp disable <id> [--global] [--harness <name>] [--json]

# Explicit state command:
orquestra mcp set-state <id> <on|off> [--global] [--harness <name>] [--json]
```

**Flags:**

- `--global`: Apply change to global baseline (default if no target is specified).
- `--harness <name>`: Apply change to a specific harness target.
- `--json`: Output updated server state as JSON.

---

### 3.7 `orquestra mcp deploy`

Recompiles and projects the master catalog (`mcps.json`) into all installed harness configuration files.

```bash
orquestra mcp deploy [OPTIONS]
```

**Options:**

- `--json`: Output report detailing modified files and target counts.

**Human Output Example:**

```text
MCP deployment completed:
  Updated: ~/.config/Cursor/mcp.json
  Updated: ~/.config/Claude/claude_desktop_config.json
  Unchanged: ~/.gemini/settings.json
  Total synced harnesses: 3
```

---

### 3.8 `orquestra mcp targets`

Lists all supported AI harness targets, their local installation status, and configuration file paths.

```bash
orquestra mcp targets [OPTIONS]
```

**Options:**

- `--json`: Output targets array as JSON.

**Human Output Example:**

```text
TARGET      STATUS         CONFIG PATH
claude      Installed      ~/.config/Claude/claude_desktop_config.json
cursor      Installed      ~/.config/Cursor/mcp.json
gemini      Installed      ~/.gemini/settings.json
codex       Not Installed  ~/.codex/mcp.json
windsurf    Installed      ~/.codeium/windsurf/mcp_config.json
opencode    Not Installed  ~/.config/opencode/mcp.json
```

---

### 3.9 `orquestra mcp test-connection <id>`

Validates whether an MCP server is reachable and responsive without opening an interactive agent session.

```bash
orquestra mcp test-connection <id> [OPTIONS]
```

**Diagnostics Executed:**

1. Verifies that the command executable exists and has executable permissions.
2. Resolves environment variables and Vault references.
3. Spawns stdio process and performs MCP protocol handshake (`initialize` / `ping`).
4. For SSE/HTTP servers, executes health check probe against target URL.

**Options:**

- `--timeout <ms>`: Timeout in milliseconds (default: `5000`).
- `--json`: Return structured diagnostics result in JSON.

**Human Output Example:**

```text
Server 'orquestra-idea-vault': OK
  Protocol Version: 2024-11-05
  Server Implementation: idea-vault v0.4.1
  Roundtrip Latency: 14ms
  Status: Healthy
```

---

### 3.10 `orquestra mcp tools-list <id>`

Discovers and lists all tools exposed by the specified MCP server using the `tools/list` protocol method.

```bash
orquestra mcp tools-list <id> [OPTIONS]
```

**Options:**

- `--timeout <ms>`: Request timeout in milliseconds (default: `5000`).
- `--json`: Output full tool schemas (including JSON Schema parameters).

**Human Output Example:**

```text
Tools for 'orquestra-idea-vault' (4 tools found):
  • record_idea: Capture a new idea in the local SQLite vault
  • list_ideas: Query ideas with optional filters by project, status, limit
  • update_idea_status: Transition workflow state (todo, in_progress, completed)
  • post_board_note: Post an inter-agent note to the shared blackboard
```

---

### 3.11 `orquestra mcp call-tool <id> <tool-name>`

Directly executes an MCP tool via `tools/call` JSON-RPC request and outputs the result.

```bash
orquestra mcp call-tool <id> <tool-name> [OPTIONS]
```

**Options:**

- `--args <json>`: Tool arguments formatted as JSON string (default: `{}`).
- `--timeout <ms>`: Execution timeout in milliseconds (default: `15000`).
- `--json`: Output complete JSON-RPC response envelope.

**Example:**

```bash
orquestra mcp call-tool orquestra-idea-vault list_ideas --args '{"project":"OrquestraAI","limit":2}'
```

**Human Output Example:**

```text
Tool 'list_ideas' succeeded:
[
  { "id": 14, "project": "OrquestraAI", "title": "CLI: orquestra mcp granular control", "status": "todo" },
  { "id": 12, "project": "OrquestraAI", "title": "Session Relay SQLite cleanup", "status": "completed" }
]
```

---

## 4. Exit Codes & Scripting Automation

The CLI follows standard Unix exit code conventions:

- `0`: Success. Operation completed cleanly.
- `1`: Operation Error. Runtime failure (e.g. server execution timeout, invalid tool parameters, network failure).
- `2`: Usage Error. Missing required arguments, invalid flags, or unrecognized subcommands.

### Scripting with `jq` Example:

```bash
# Find all tools across active servers that support project management:
for server in $(orquestra mcp list --enabled-only --json | jq -r '.[].id'); do
  echo "=== Tools in $server ==="
  orquestra mcp tools-list "$server" --json | jq -r '.tools[].name'
done
```

---

## 5. Security & Isolation Considerations

1. **Vault Encryption:** The catalog file `mcps.json` is protected by AES-256-GCM. Manual filesystem inspection will show an encrypted envelope. Always use `orquestra mcp inspect <id> --reveal` from an authenticated terminal to inspect decrypted credentials.
2. **Path Sanitization:** Executable paths and arguments undergo strict escaping and argument array passing (`Command::args`) to prevent shell injection vulnerabilities.
3. **Non-Interactive Guardrail:** Remote tool invocations run within isolated subprocesses without allocating pseudo-terminals or inherited stdin unless explicitly requested.
