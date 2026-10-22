# OrquestraAI Architecture

Single-binary Tauri 2 desktop app. A React shell owns all UI state; a Rust core
owns all disk, process and secret access. The two meet at exactly one boundary:
the IPC protocol in `docs/IPC_API.md` (66 commands, request/response, no
backend-pushed events).

```
React shell (src/)                    Rust core (src-tauri/src/)
views - hooks - lib/*Cache.ts         lib.rs handler registry
        |  invoke("<command>", args)          |
        +------------------------------------>+
        <-------- JSON (camelCase) ------------+
```

## Module map

| Area           | Frontend                                                  | Backend                                                      | Storage                                                                        |
| -------------- | --------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------ |
| Skills         | `SkillsView`, `useSkillsCache`, `skillsLoader`            | `skills/` (vault, projection, security, agents, vault_cache) | `~/.orquestra/skills/<id>/`, projections under `~/.orquestra/harnesses/<hid>/` |
| MCP            | `McpView`, `useMcpCatalog`, `useMcpMutations`, `mcpCache` | `mcp/` (catalog, deploy, serializers)                        | `mcps.json` catalog; compiled harness configs                                  |
| Presets        | `PresetEditModal`, presets bar                            | `presets/` (store, apply, builtins, naming)                  | `presets.json` (`custom` + `overrides`)                                        |
| Bridge         | footer indicator                                          | `bridge/` (status, deploy, fuse)                             | symlinks + `backupDir` per deploy                                              |
| Ideas          | `IdeasView`, `useIdeas`                                   | `ideas/` (store, ideas_repo, notes_repo, productivity)       | `ideas.db` (SQLite)                                                            |
| Sessions       | `SessionsView`, `TranscriptFeed`, `useSessions`           | `sessions/` (adapters/*, policy, purge, scheduler)           | harness-native histories (read-only) + pins/policy                             |
| Vault          | Credentials view, `AddCredentialModal`, `vault.ts`        | `vault/` (store, proxy, exec, http, presets, atomic)         | `vault.json` (`0600`), atomic writes                                           |
| Updates        | `updater.ts`, settings tab                                | `updater.rs`, `update_sig.rs`                                | staged in scratch dir, atomic replace                                          |
| System         | Janitor modal, settings                                   | `janitor/`, `desktop.rs`, `tray.rs`                          | sockets, `backups/`                                                            |
| Official store | store UI                                                  | `official.rs`                                                | static curated data                                                            |

Cross-cutting: `paths.rs` (single `OrquestraPaths` resolver + bootstrap),
`log.rs`, `single_instance.rs`, `cli.rs`, `ipc_contract.rs` + fixture
(contract tests), `ipc_acl.rs` + `build.rs` (deny-by-default ACL).

## Startup sequence (`lib.rs run()`)

1. `OrquestraPaths::resolve().bootstrap()` — create `~/.orquestra` layout.
   Failure is logged, never fatal.
2. `desktop::ensure_desktop_integration()` — Linux launcher/dock entry.
3. `tray::setup_tray()` — system tray; failure tolerated (window still opens).
   `AppConfigState.minimize_to_tray` (set via `set_minimize_to_tray`) decides
   whether close hides to tray or exits.
4. `single_instance::start_listener()` — second launches focus this window
   instead of spawning a duplicate process + tray icon.
5. `sessions::start_scheduler()` — retention daemon; enforces `autoPurge` on
   its interval, never touches pinned or open sessions.
6. Window event: `CloseRequested` routes through `tray::on_close_requested`.

Release builds additionally verify `dist/index.html` exists via `build.rs`,
auto-triggering `npm run build` so a backend-only build cannot ship a stale
(or missing) frontend.

## Invoke flow

1. React calls `invoke("<command>", snake_caseArgs)` (typed wrappers live in
   `src/lib/tauri.ts`, `src/lib/vault.ts`, `src/lib/ideas.ts`, …).
2. Tauri deserializes args into the handler's Rust parameters and runs the
   `#[tauri::command]` fn (or its async variant) from the `generate_handler!`
   registry.
3. The handler returns `T` or `Result<T, String>`; Tauri serializes to
   `camelCase` JSON. `Err(string)` surfaces to the caller as a rejected promise.
4. The ACL layer denies any command missing from the window's capability
   scopes before the handler runs (see `docs/IPC_API.md § Authorization scopes`).

Frontend caching: `mcpCache`, `agentsCache`, `useSkillsCache` and friends hold
read results; mutations re-invoke the list command (or bump a refresh counter)
so views never serve stale state. There is no invalidation protocol because
there are no pushed events — freshness comes from re-fetch after write.

## Bridge harness lifecycle

The bridge makes every supported harness (codex, claude, copilot, …) see the
same OrquestraAI-managed skills, MCP configs and instructions.

- **Detect** (`get_bridge_status`): for each known harness id, check
  `installed` (native dir exists?), `supported`, and whether the skills /
  config / instructions links point at the projection. Secondary files carry a
  stored action (`adoptOnly` | `project` | `quarantine`) plus a `cleared` flag.
- **Deploy** (`deploy_harness_bridge`): `paths.bootstrap()`, then per harness —
  back up the native dir into `backupDir`, symlink skills/config/instructions
  into `~/.orquestra/harnesses/<id>/`, and apply secondary actions (adopt
  into the vault, quarantine aside, or clear). The `BridgeReport` counts
  `total / bridged / adopted / quarantined / instructionsImported` and carries
  `warnings[]` / `errors[]`; originals are always recoverable from `backupDir`.
- **Steady state**: the footer indicator polls `get_bridge_status`; skill/MCP
  mutations recompile projections and configs without re-bridging (vault
  folders refresh in place so symlinks stay live).

## Sessions pipeline

Per-harness adapters (`sessions/adapters/`: codex, claude, copilot, grok,
antigravity, freebuff, freebuff_desktop, + `transcript`, `utils`, `fsops`)
translate native histories into `UnifiedSession` / `TranscriptTurn`. Reads are
async (worktree sizing walks `node_modules`). OrquestraAI-owned layers on top:

- **pins** (`policy::toggle_pin`) overlaid as `isPinned`/`pinLabel`;
- **retention** (`get/save_retention_policy`, defaults on first run);
- **purge** (`estimate_purge` dry-run → `purge_sessions`), which keeps pinned
  sessions unless explicitly included and always skips sessions a running
  agent holds open;
- **scheduler**: the background daemon enforcing `autoPurge`.

### Adapter native formats

Every adapter translates one harness's on-disk layout into `UnifiedSession`
listing rows and `TranscriptTurn` feeds. All paths are relative to `$HOME`;
ids are path-segment-checked so `../` escapes and symlinks pointing outside
the store resolve to an empty feed, never to outside bytes.

| Harness     | Store                                                                                               | Listing source                                                                                                                                                                                            | Transcript source                                                                                 |
| ----------- | --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| claude      | `~/.claude`                                                                                         | `history.jsonl` (`sessionId`, `display`, `project`, `timestamp`\|`created_at` as epoch or ISO) ∪ `projects/<slug>/<id>.jsonl` headers (first user prompt + recorded `cwd`) + legacy `sessions/<id>.jsonl` | Per-line JSONL transcript; Manicode-style single-array files read element-wise                    |
| codex       | `~/.codex`                                                                                          | `session_index.jsonl` (`id`, `thread_name`\|`title`, `updated_at`) ∪ `sessions/YYYY/MM/DD/rollout-*.jsonl` + turn counts/activity from read-only `thread_history_1.sqlite`                                | Rollouts streamed line by line, keeping only `type: response_item` with `user`\|`assistant` roles |
| copilot     | `~/.copilot`                                                                                        | `session-store.db` (SQLite) + `open-sessions-state.json` registry                                                                                                                                         | Turns built from DB rows (role/content/timestamp)                                                 |
| grok        | `~/.grok`                                                                                           | `sessions/<encoded-cwd>/<id>/` dirs + `active_sessions.json` for open sessions; `session_search.sqlite` for search                                                                                        | `chat_history.jsonl` per session dir                                                              |
| antigravity | `~/.gemini`                                                                                         | `antigravity/brain/<id>/` dirs + `conversations/<id>.db`; costs in `conversation_summaries.db`                                                                                                            | `brain/<id>/…/logs/transcript.jsonl` (newer nests under `.system_generated/`)                     |
| hermes      | `~/.hermes`                                                                                         | `sessions/<id>.jsonl` + `terminal-sessions/<id>.jsonl`; `state.db`                                                                                                                                        | Same JSONL files, lines → turns                                                                   |
| opencode    | `~/.local/share/opencode/opencode.db`                                                               | SQLite via shared `sqlite_store`                                                                                                                                                                          | Same DB: join/message/part table layouts tried in order                                           |
| kilo        | `~/.local/share/kilo/kilo.db`                                                                       | SQLite via shared `sqlite_store`                                                                                                                                                                          | Same DB, same fallback order                                                                      |
| freebuff    | Manicode chat logs + `~/.config/freebuff-desktop/local-projects/<slug>/threads/<thread>/state.json` | Log files / thread state                                                                                                                                                                                  | JSON-array chat logs via the shared reader; desktop threads via `freebuff_desktop`                |

Shared mapping (`sessions/adapters/transcript*.rs`, used by every reader):
`parse_value` detects the canonical role, extracts text blocks and ISO
timestamps; `build_turn` constructs the turn; `clean_turn_content` +
`transcript_sanitize` strip tool-run noise so the drawer shows chat bubbles.
Caps are uniform — first 1,000 turns, 12,000 chars per turn (trimmed, never
dropped) — corrupt lines are skipped, and purge deletes only inside the store
(`fsops::remove_path`) with SQLite `vacuum` afterwards.

## Vault trust model

- `vault.json` is `0600`, written atomically; backups embed cleartext secrets
  and are locked to `0700`/`0600`.
- IPC crossings are masked by construction (`VaultServicePublic`).
- Secrets are _used_, never _returned_: `vault_http_request` injects the active
  key, fails over the pool on 401/402/403/429, strips auth response headers and
  scrubs secrets from the body; `vault_exec` allowlists bare binary names with
  no shell and scrubs output.
- Both blind calls sit in `vault-blind-ops`, withheld from every window until
  a confirmed UI caller exists; `reveal_mcp_secret` (one value, explicit click)
  is the only path that returns a secret to the UI.

## Updater flow

`checkLatestRelease` (frontend, GitHub releases API) → `apply_app_update`
(download → SHA-256 verify in a self-cleaning `Staging` dir → atomic replace)
→ `restart_app` (socket file removed _before_ spawning so the child
deterministically becomes primary). `update_sig.rs` owns signature
verification; `sign-release.py` + `scaffold-extensions-monorepo.sh` are
maintainer-side scripts, not runtime code.

## Contract safety net

Three layers keep the single IPC boundary honest:

1. `tests/fixtures/ipc-contract.json` — frozen command list, CLI flags, ACL
   scope split and payload key shapes.
2. `ipc_contract.rs` tests — handler list, frontend `invoke(` call sites and
   serialized key sets must match the fixture.
3. `scripts/ipc-docs.test.mjs` — every registered command must have an entry
   in `docs/IPC_API.md`, so this documentation cannot drift from the protocol.
