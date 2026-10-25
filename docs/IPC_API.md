# OrquestraAI IPC API

Wire protocol between the React shell (`src/`) and the Rust core (`src-tauri/src/`),
spoken through Tauri `invoke("<command>", args)`.

- **66 commands**, all registered in one `tauri::generate_handler![…]` list in
  `src-tauri/src/lib.rs`. The frozen list lives in
  `src-tauri/tests/fixtures/ipc-contract.json` (`commands`); `cargo test`
  fails if a command is added, renamed or removed without updating the fixture.
- **Request/response only.** The backend never pushes events — there is no
  `emit`/`listen` traffic. The UI refreshes by re-invoking after a mutation
  (most views go through the caches in `src/lib/*Cache.ts` / `src/hooks/*`).
- **Argument names are Rust `snake_case`** and must match the parameter names
  below exactly (e.g. `{ harness_id: "global", enabled: true }`).
- **Errors are plain strings** (`Result<_, String>`). Success payloads serialize
  with `camelCase` keys. Exact key sets for the main shapes are frozen in the
  fixture under `payloadShapes`; the tables below describe semantics, not keys.
- **Async commands** (`get_unified_sessions`, `get_session_transcript`,
  `estimate_purge`, `purge_sessions`, `pick_folder`, `vault_http_request`) run
  on the async runtime so megabyte transcripts and multi-second scans never
  freeze the window. Everything else runs synchronously on the Tauri thread.

## Authorization scopes

`src-tauri/build.rs` denies every command by default; each command must sit in
exactly one scope (`src-tauri/permissions/*.toml`), granted to the main window
by `src-tauri/capabilities/*.json`. Destructive commands must never move into
`safe-read` — the contract test enforces the split.

| Scope             | Grants                                                                              | Commands                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ----------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `safe-read`       | reads, dialogs, backup                                                              | `core_status`, `log_ui_error`, `get_system_health`, `list_skills`, `read_skill_content`, `harness_summary`, `read_master_agents_md`, `get_bridge_status`, `list_mcps`, `list_official_store`, `mcp_targets`, `list_presets`, `list_ideas`, `list_idea_projects`, `get_productivity_metrics`, `get_unified_sessions`, `get_session_transcript`, `list_blackboard_notes`, `get_retention_policy`, `list_vault_presets`, `vault_list_services`, `estimate_purge`, `pick_folder`, `open_url`, `open_in_file_manager`, `create_orquestra_backup`, `set_minimize_to_tray` |
| `vault-ops`       | masked vault + secret reveal                                                        | `vault_save_service`, `vault_delete_service`, `vault_set_primary`, `vault_set_service_enabled`, `vault_test_connection`, `vault_remove_key`, `reveal_mcp_secret`                                                                                                                                                                                                                                                                                                                                                                                                    |
| `vault-blind-ops` | secret-_using_ calls, withheld from every window until a confirmed UI caller exists | `vault_http_request`, `vault_exec`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `update-ops`      | self-update, restart                                                                | `apply_app_update`, `sync_official_extensions`, `restart_app`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `session-ops`     | pins, retention, purge                                                              | `purge_sessions`, `save_retention_policy`, `toggle_pin_session`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `admin-ops`       | everything that writes projections, catalogs, ideas, notes                          | `sweep_system_caches`, `set_skill_state`, `batch_set_all`, `save_skill_content`, `delete_skill`, `import_skill`, `import_skill_from_git`, `write_master_agents_md`, `deploy_harness_bridge`, `set_mcp_state`, `batch_set_mcp_all`, `save_mcp`, `delete_mcp`, `deploy_mcps`, `apply_preset`, `save_preset`, `delete_preset`, `update_preset`, `reset_preset`, `create_idea`, `update_idea`, `update_idea_status`, `delete_idea`, `post_blackboard_note`, `mark_note_read`                                                                                            |

## Core & shell

| Command                   | Args                                                | Returns                               | Side effects / notes                                                                                                                                                                                           |
| ------------------------- | --------------------------------------------------- | ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `core_status`             | —                                                   | `{ name, version, platform, bridge }` | Pure. `bridge` is always `"ready"`; per-harness state is `get_bridge_status`.                                                                                                                                  |
| `log_ui_error`            | `area: string`, `message: string`, `stack?: string` | `null`                                | Writes to stderr next to backend logs. Called by `ErrorBoundary`.                                                                                                                                              |
| `pick_folder`             | —                                                   | `string \| null`                      | Opens the native folder picker. `null` = user cancelled. Async.                                                                                                                                                |
| `open_url`                | `url: string`                                       | `null`                                | Opens an external `http(s)` URL in the system browser. Non-URL input is refused.                                                                                                                               |
| `open_in_file_manager`    | `path: string`                                      | `null`                                | Reveals a path inside `~/.orquestra` in the file manager. `~` resolves against `HOME` (falling back to `USERPROFILE`); anything outside the root or nonexistent is refused.                                    |
| `create_orquestra_backup` | —                                                   | archive path `string`                 | Snapshots `~/.orquestra` (configs, presets, vault, projections) to `~/.orquestra/backups/*.tar.gz`, excluding previous backups. Dir/file locked to `0700`/`0600` — the tarball embeds cleartext vault secrets. |
| `set_minimize_to_tray`    | `enabled: boolean`                                  | `null`                                | Stores the close-behavior flag the tray module reads on window close. Called on startup and on every settings toggle.                                                                                          |

## Skills

Skill state is _projected_: the vault (`~/.orquestra/skills/`) is the source of
truth; enabling a skill symlinks it into each harness projection folder.

| Command                  | Args                                                   | Returns                                      | Side effects / notes                                                                                               |
| ------------------------ | ------------------------------------------------------ | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `list_skills`            | `harness_id?: string`                                  | `SkillItem[]`                                | `None` evaluates the universal Global baseline. Pure read.                                                         |
| `set_skill_state`        | `id: string`, `harness_id: string`, `enabled: boolean` | `null`                                       | Projects/un-projects one skill. Scope `"global"` propagates to every harness folder; harness scopes stay isolated. |
| `batch_set_all`          | `harness_id: string`, `enabled: boolean`               | `null`                                       | Projects/un-projects every skill for one scope.                                                                    |
| `read_skill_content`     | `id: string`                                           | raw `SKILL.md` `string`                      | Pure read for the in-app editor.                                                                                   |
| `save_skill_content`     | `id: string`, `content: string`                        | `null`                                       | Atomically replaces the skill's `SKILL.md`.                                                                        |
| `delete_skill`           | `id: string`                                           | `null`                                       | Unlinks the skill from every scope and removes its vault folder.                                                   |
| `import_skill`           | `source_path: string`                                  | `SkillItem`                                  | Copies an external skill folder into the vault. Reads via the folder picker or a pasted path.                      |
| `import_skill_from_git`  | `url: string`                                          | `SkillItem`                                  | Shallow-clones a git URL and imports the skill it holds.                                                           |
| `harness_summary`        | —                                                      | `HarnessSummary[]` (`{ id, active, total }`) | Active-skill counters for Global and every harness tab. Pure.                                                      |
| `read_master_agents_md`  | —                                                      | `string`                                     | Reads `~/.orquestra/AGENTS.md`, creating a template on first use.                                                  |
| `write_master_agents_md` | `content: string`                                      | `null`                                       | Atomically replaces the master `AGENTS.md`.                                                                        |

## MCP servers

The catalog (`mcps.json`) is the source of truth; `deploy` compiles it into each
installed harness's native config format. Secrets in `env`/headers stay masked
in every read — only `reveal_mcp_secret` returns one cleartext value, after an
explicit UI click.

| Command               | Args                                                    | Returns                                                      | Side effects / notes                                                                                                    |
| --------------------- | ------------------------------------------------------- | ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------- |
| `list_mcps`           | —                                                       | `McpServer[]` (masked)                                       | Whole master catalog, seeded on first launch.                                                                           |
| `list_official_store` | —                                                       | `OfficialStore` (`{ skills[], mcps[] }`)                     | Curated registry with monorepo anchors + fallbacks. Static data, infallible.                                            |
| `reveal_mcp_secret`   | `id: string`, `key: string`                             | cleartext `string`                                           | Returns one stored env/header value (or the server `url`). Vault-ops scoped.                                            |
| `mcp_targets`         | —                                                       | `McpTargetStatus[]` (`{ id, label, configPath, installed }`) | Harness configs the serializer can write.                                                                               |
| `set_mcp_state`       | `id: string`, `harness_id?: string`, `enabled: boolean` | `null`                                                       | `None` = global scope; `Some` = one harness. Recompiles configs.                                                        |
| `batch_set_mcp_all`   | `harness_id?: string`, `enabled: boolean`               | `null`                                                       | One atomic write for a whole scope, then recompile.                                                                     |
| `save_mcp`            | `mcp_server: McpServer`                                 | `null`                                                       | Insert/replace + recompile. The `official` flag is recomputed server-side from the id — client state is ignored.        |
| `delete_mcp`          | `id: string`                                            | `null`                                                       | Drops the server; its id lingers in `managedIds` just long enough for the deploy to strip it from every harness config. |
| `deploy_mcps`         | —                                                       | changed config paths `string[]`                              | Recompiles the catalog into every installed harness config; files that now carry env values are tightened to `0600`.    |

## Presets

Named snapshots of a skill+MCP combination (built-ins ship compiled in; customs
live in `~/.orquestra/presets.json` with per-built-in `overrides`).

| Command         | Args                                   | Returns                                                                                       | Side effects / notes                                                                                                                                            |
| --------------- | -------------------------------------- | --------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_presets`  | —                                      | `Preset[]`                                                                                    | Built-ins + customs, applied one flagged. Pure.                                                                                                                 |
| `apply_preset`  | `id: string`                           | `PresetReport` (`{ skillsEnabled/Disabled, mcpsEnabled/Disabled, configsChanged, warnings }`) | Applies to projections + catalog. Unknown skill/MCP ids are ignored with a warning so a preset survives a different machine's vault.                            |
| `save_preset`   | `name: string`, `description?: string` | `Preset`                                                                                      | Snapshots the _live_ skill+MCP combination as a custom profile. Empty name is an error.                                                                         |
| `delete_preset` | `id: string`                           | `null`                                                                                        | Customs only — built-ins are compiled in and refuse.                                                                                                            |
| `update_preset` | `preset: Preset`                       | `Preset`                                                                                      | Full-object update; built-in ids land in `overrides`, custom ids in `custom`. Scope maps are normalized so hand-edited payloads cannot store unknown harnesses. |
| `reset_preset`  | `id: string`                           | the factory `Preset`                                                                          | Discards a built-in's override. Non-built-in ids are an error.                                                                                                  |

## Bridge

Symlinks each installed harness (`~/.<harness>`) into its OrquestraAI
projection (`~/.orquestra/harnesses/<id>/`). Full lifecycle in
`docs/ARCHITECTURE.md § Bridge harness lifecycle`.

| Command                 | Args | Returns                                                                                                            | Side effects / notes                                                                                                                                                        |
| ----------------------- | ---- | ------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `get_bridge_status`     | —    | `HarnessStatus[]`                                                                                                  | Live link state per harness (`bridged`, `installed`, `supported`, link flags, secondary clearances, notes). Pure; feeds the footer indicator.                               |
| `deploy_harness_bridge` | —    | `BridgeReport` (`{ total, bridged, adopted, quarantined, instructionsImported, backupDir, warnings[], errors[] }`) | Backs up each harness dir, then symlinks skills/config/instructions. Secondary files are adopted / quarantined / cleared per stored actions; originals land in `backupDir`. |

## Ideas & blackboard

SQLite-backed (`ideas.db`): an idea vault plus an inter-agent blackboard.
`status`/`category`/`priority` fall back to MCP defaults (`open`/`feature`/
`medium`) when omitted. List endpoints page via `limit`/`offset`
(`store::clamp_page`); omitting them returns the default first page.

| Command                    | Args                                                                              | Returns                                                                                              | Side effects / notes                                                               |
| -------------------------- | --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `list_ideas`               | `project?: string`, `status?: string`, `limit?: number`, `offset?: number`        | `IdeaItem[]`, highest priority first                                                                 | Missing `status` filter defaults to `open`.                                        |
| `list_idea_projects`       | —                                                                                 | `string[]`                                                                                           | Distinct project names for the filter dropdown.                                    |
| `create_idea`              | `input: { project*, title*, description?, category?, priority?, tags?, prompt? }` | `IdeaItem`                                                                                           | `*` required. Unknown enum values are errors.                                      |
| `update_idea`              | `input: { id*, …same optionals, status? }`                                        | `IdeaItem`                                                                                           | Partial update — only `Some` fields are written; `updated_at` always moves.        |
| `update_idea_status`       | `id: number`, `status: string`                                                    | `IdeaItem`                                                                                           | Convenience move to a new status.                                                  |
| `delete_idea`              | `id: number`                                                                      | `null`                                                                                               | Unknown id is an error.                                                            |
| `get_productivity_metrics` | `project?: string`, `weeks?: number`                                              | `ProductivityMetrics` (`{ currentStreak, avgCycleHours, weeklyVelocity, dailyActivity[], project }`) | Vault completions + local git commits per day.                                     |
| `list_blackboard_notes`    | `project?: string`, `unread_only: boolean`, `limit?: number`, `offset?: number`   | `BlackboardNote[]`, oldest first                                                                     | Inter-agent notes; `target` defaults to `all` at post time, `topic` is lowercased. |
| `post_blackboard_note`     | `input: { project*, topic*, sender*, target?, note* }`                            | `BlackboardNote`                                                                                     | Posts a note for other agents.                                                     |
| `mark_note_read`           | `id: number`                                                                      | `null`                                                                                               | Unknown id is an error.                                                            |

## Sessions

Unified read over every harness's native chat history via per-harness adapters
(`codex`, `claude`, `copilot`, `grok`, `antigravity`, `freebuff`, …) plus a
pin/retention/purge layer owned by OrquestraAI.

| Command                  | Args                                                                                     | Returns                                                                                                                     | Side effects / notes                                                                                                                                                                  |
| ------------------------ | ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `get_unified_sessions`   | —                                                                                        | `UnifiedSession[]` (`{ id, harness, title, projectPath, turnsCount, sizeBytes, createdAt, updatedAt, isPinned, pinLabel }`) | Async (worktree sizing walks `node_modules`, ~seconds). Pins are overlaid from OrquestraAI storage.                                                                                   |
| `get_session_transcript` | `harness: string`, `session_id: string`                                                  | `TranscriptTurn[]` (`{ role, content, timestamp }`)                                                                         | Async — transcripts run to megabytes. Unknown harness resolves to an empty feed (in-development note) instead of an error.                                                            |
| `toggle_pin_session`     | `harness: string`, `session_id: string`, `label?: string`                                | new pinned state `boolean`                                                                                                  | Pins survive retention purges unless explicitly included. Session-ops scoped.                                                                                                         |
| `get_retention_policy`   | —                                                                                        | `RetentionPolicy` (defaults on first run)                                                                                   | Pure.                                                                                                                                                                                 |
| `save_retention_policy`  | `policy: RetentionPolicy`                                                                | `null`                                                                                                                      | Replaces the rules; the pin list is preserved. Session-ops scoped.                                                                                                                    |
| `estimate_purge`         | `filter: { harness?, olderThanDays?, pinnedOnly?, search?, sessionIds?, ignorePinned? }` | `PurgeEstimate` (`{ sessionsCount, reclaimedBytes, affectedHarnesses, skippedPinnedCount, skippedActiveCount }`)            | Dry run — nothing on disk is touched. Async. Safe-read.                                                                                                                               |
| `purge_sessions`         | same `filter`                                                                            | `PurgeReport` (`{ deletedCount, failedCount, reclaimedBytes, errors[] }`)                                                   | Deletes matches. Pinned sessions are kept unless the filter explicitly asks for them; a session a running agent holds open is always skipped and reported. Session-ops scoped. Async. |

A retention daemon (`sessions::start_scheduler`, spawned once at startup)
enforces `autoPurge` on `autoPurgeIntervalHours` and never touches pinned or
open sessions.

## Credential vault (blind)

Services hold 1+ pooled keys. Reads crossing IPC are always masked
(`VaultServicePublic`: `masked_keys`, `active_key_index`, `enabled`,
per-key `health`, `key_tags`, `cooldown_until_ms`, non-secret `meta`).
Secrets are only ever _used_ server-side by the two blind calls — they
never appear in a result. Disabled services stay listed but the blind
calls refuse them, so agents only ever reach enabled services.

| Command                         | Args                                                                                                                                                                                              | Returns                                                       | Side effects / notes                                                                                                                                                                                                                   |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_vault_presets`            | —                                                                                                                                                                                                 | `PresetSpec[]` (AI providers, cloud/infra, dev tools, custom) | Static catalog for the drawer: header/prefix/env shaping, key URL, meta fields. Pure.                                                                                                                                                  |
| `vault_list_services`           | —                                                                                                                                                                                                 | `VaultServicePublic[]`                                        | Masked: names, active index, counts, health. No secrets.                                                                                                                                                                               |
| `vault_save_service`            | `id, label, preset: string`, `description?, custom_header?, custom_prefix?, custom_env?: string`, `keys: string[]`, `meta?: map`, `key_tags?: (string \| null)[]`, + saved-key preservation flags | `VaultServicePublic`                                          | Register/replace a service with 1+ keys. Masked placeholders for existing keys are preserved so the drawer round-trips safely.                                                                                                         |
| `vault_delete_service`          | `id: string`                                                                                                                                                                                      | `null`                                                        | Removes one service.                                                                                                                                                                                                                   |
| `vault_set_primary`             | `id: string`, `index: number`                                                                                                                                                                     | `VaultServicePublic`                                          | Designates the primary key index for failover order.                                                                                                                                                                                   |
| `vault_set_service_enabled`     | `id: string`, `enabled: boolean`                                                                                                                                                                  | `VaultServicePublic`                                          | Operator kill-switch: disabled services stay listed (masked) but `vault_http_request`, `vault_exec` and `vault_test_connection` refuse them. Vault-ops scoped.                                                                         |
| `vault_remove_key`              | `id: string`, `index: number`                                                                                                                                                                     | `VaultServicePublic`                                          | Removes one key from a pool; re-indexes and keeps a valid primary.                                                                                                                                                                     |
| `vault_test_connection`         | `service: string`, `key_index?: number`                                                                                                                                                           | `{ ok, latencyMs, … }` (`VaultTestResult`)                    | Pings the preset's verify endpoint with one pooled key, persists the observed health. Secret never in the result. Async.                                                                                                               |
| `vault_purge_migration_backups` | —                                                                                                                                                                                                 | `{ vaultPurged, catalogPurged }` (`PurgeBackupsReport`)       | Zeroizes (overwrites + syncs) and unlinks `vault.json.migrate-bak` / `mcps.json.migrate-bak`. Only purges a backup whose sealed file still opens; missing backups report `false`. Vault-ops scoped, explicit UI confirmation.          |
| `vault_http_request`            | `service: string`, `endpoint: string`, `method?: string`, `headers?: map`, `body?: string`, `key_tag?: string`                                                                                    | `{ status, body, usedKeyIndex, rotated }` (`VaultHttpResult`) | Blind HTTP: injects the active key, fails over across the pool on 401/402/403/429, strips auth response headers and scrubs secrets from the body. Async. Withheld from every window (`vault-blind-ops`).                               |
| `vault_exec`                    | `service: string`, `command: unknown (must be string[])`                                                                                                                                          | `{ code, stdout, stderr, usedKeyIndex }` (`VaultExecResult`)  | Blind exec: `argv[0]` must be a bare name on the allowlist (`exec::ALLOWED_BINARIES`), no shell — shell strings, pipes and redirections are rejected. Secrets are env-injected and scrubbed from output. Withheld (`vault-blind-ops`). |

## System health & updates

| Command                    | Args                     | Returns                                                            | Side effects / notes                                                                                                                                               |
| -------------------------- | ------------------------ | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `get_system_health`        | —                        | `SystemHealth` (`{ ram, swap, cpu, tmp, topProcesses[], alert? }`) | Live `/proc` snapshot. Explicit error off Linux.                                                                                                                   |
| `sweep_system_caches`      | —                        | `SweepReport` (`{ removedCount, reclaimedMb, status }`)            | Removes only dead Orquestra socket files and aged direct-child scratch files. Never touches user data.                                                             |
| `apply_app_update`         | `target_version: string` | human message `string`                                             | Downloads the release binary, verifies SHA-256, atomically replaces the running executable. Version must be newer semver. Update-ops scoped.                       |
| `sync_official_extensions` | —                        | `ExtensionSyncResult` (`{ updated_skills[], updated_mcps[] }`)     | Pulls upstream into every installed official skill with a known repo URL. Vault folders refresh in place so projection symlinks stay live — no re-projection pass. |
| `restart_app`              | —                        | `null` (never resolves on success)                                 | Respawns the active binary and exits. The single-instance socket file is removed _before_ spawning so the child deterministically becomes primary.                 |

## Headless CLI (same backend, no window)

`src-tauri/src/cli.rs` + `main.rs` expose the same backend for scripts. Flags
(frozen in the fixture under `cliFlags`):

`--preset <id>` → `apply_preset_json` · `--mcp-status` → `mcp_status_json` ·
`--mcp-deploy` → `deploy_mcp_json` · `--mcp-set <id=harness=bool>` →
`set_mcp_state_cli` · `--bridge-status` → `bridge_status_json` ·
`--bridge-deploy` → `deploy_bridge_json` · `--mcp-status` in release builds also
serves `--mcp-status` health probes used by `scripts/check-card-motion.mjs`.
`--help`/`-h` prints usage.

Native subcommands (`src-tauri/src/cli/native/`, no window) follow the same
backend: `ideas`, `sessions`, `bridge status` and `vault`. The vault one
exists so the `orquestra-core` MCP server never reads `vault.json` off disk
(sealed since alpha.5) — it delegates instead:

`vault list [--json]` → masked `VaultServicePublic[]` (ids, labels,
`enabled`, health, key counts; never secrets) ·
`vault set-enabled <id> <on|off> [--json]` → flips the kill-switch; disabled
services stay listed but the blind calls refuse them ·
`vault http --service <id> --endpoint <url> [--method …] [--headers …] [--body …] [--key-tag …]` →
scrubbed `VaultHttpResult` JSON ·
`vault exec <service> [--] <argv…>` → scrubbed `VaultExecResult` JSON.
`http`/`exec` refuse disabled services exactly like the IPC blind calls.

## Worked examples

Wire keys are `snake_case` (Tauri deserializes them into the Rust parameter
names); success payloads come back `camelCase`. Secrets never appear in any
response — leaked key material is replaced with `[REDACTED]`.

### `vault_http_request` (blind, failover)

```jsonc
// invoke("vault_http_request", …)
{
  "service": "openai",
  "endpoint": "https://api.openai.com/v1/models",
  "method": "GET"
}
// → { status, headers, body, usedKeyIndex, rotated }
{
  "status": 200,
  "headers": { "content-type": "application/json" },
  "body": "{\"data\":[]}",
  "usedKeyIndex": 0,
  "rotated": false
}
```

When the active key is rejected (401/402/403/429) the core retries the next
pooled key and reports which one worked:

```jsonc
// invoke("vault_http_request", …) — first key expired
{
  "service": "openai",
  "endpoint": "https://api.openai.com/v1/models",
  "method": "GET",
  "key_tag": "workers" // optional: restrict failover to keys with this tag
}
// →
{
  "status": 200,
  "headers": {},
  "body": "{\"data\":[]}",
  "usedKeyIndex": 1,
  "rotated": true
}
```

Auth response headers are stripped and any secret echoed in the body is
scrubbed before the response crosses IPC:

```jsonc
{ "status": 200, "headers": {}, "body": "key=[REDACTED]", "usedKeyIndex": 0, "rotated": false }
```

Failure (unknown service) rejects the promise with a plain string:

```
"unknown vault service: openai"
```

### `vault_exec` (blind, allowlisted binary)

```jsonc
// invoke("vault_exec", …) — command MUST be an array; argv[0] must be a
// bare name on the exec allowlist. No shell, no pipes, no redirects.
{
  "service": "cloudflare",
  "command": ["wrangler", "r2", "object", "list", "my-bucket"]
}
// → { code, stdout, stderr, usedKeyIndex }
{
  "code": 0,
  "stdout": "objects…(key material shown as [REDACTED])",
  "stderr": "",
  "usedKeyIndex": 0
}
```

A shell string is rejected, never executed:

```jsonc
{ "service": "cloudflare", "command": "wrangler r2 object list my-bucket" }
// → Error: "vault exec needs a command array, argv[0] must be an allowlisted binary"
```

### `vault_test_connection` (blind ping)

```jsonc
// invoke("vault_test_connection", { service: "openai" })
// → { success, latencyMs, message, health }
{
  "success": true,
  "latencyMs": 312,
  "message": "ok",
  "health": "ok",
}
// invoke("vault_test_connection", { service: "openai", key_index: 1 })
// pins the probe to one pooled key instead of the active one.
```

The observed health is persisted, so the next `vault_list_services` reflects
it in `health` / `key_health`.

### Purge flow (dry run first)

```jsonc
// 1. invoke("estimate_purge", { filter: { olderThanDays: 90 } })
{
  "sessionsCount": 41,
  "reclaimedBytes": 187291648,
  "affectedHarnesses": ["codex", "claude"],
  "skippedPinnedCount": 3,
  "skippedActiveCount": 1
}
// 2. invoke("purge_sessions", { filter: { olderThanDays: 90 } })
{
  "deletedCount": 41,
  "failedCount": 0,
  "reclaimedBytes": 187291648,
  "errors": []
}
// Pinned sessions are skipped unless the filter sets ignorePinned: true;
// a session a running agent holds open is always skipped and reported.
```

## Coverage check

`scripts/ipc-docs.test.mjs` parses the `generate_handler!` list and asserts
every command has a `` `name` `` entry in this file, so documentation cannot
drift from the protocol. Run it with `npm test`.
