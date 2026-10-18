# OrquestraAI

Universal AI Harness Administration Platform — a Tauri v2 desktop control plane
for projecting a single skills library into every AI harness.

See [PRD.md](PRD.md) for the full product specification.

## Stack

| Layer    | Technology                                             |
| :------- | :----------------------------------------------------- |
| Shell    | Tauri v2 (`src-tauri/`, Rust 2021)                      |
| Frontend | React 19 + TypeScript + Vite 8                          |
| Styling  | Tailwind CSS v4 (Zinc/Slate dark palette, emerald/cyan) |
| Icons    | `lucide-react`                                          |
| Helpers  | `clsx` + `tailwind-merge` via `cn()`                    |

## Prerequisites

- Node.js 20+ and npm
- Rust toolchain (stable) with `cargo`
- Linux: `webkit2gtk-4.1`, `libsoup-3.0`, `gtk+-3.0`, `librsvg2` (Tauri v2 runtime deps)

## Commands

```bash
npm install          # install frontend dependencies
npm run dev          # Vite dev server on http://127.0.0.1:1420
npm run build        # tsc --noEmit + production frontend build
npm run tauri dev    # run the desktop app with hot reload
npm run tauri build  # bundle a release desktop build

cargo check --manifest-path src-tauri/Cargo.toml   # Rust-only check
cargo test --manifest-path src-tauri/Cargo.toml --lib  # scope, symlink, bridge, AGENTS.md tests

# Headless bridge (same code the UI calls)
./target/release/orquestra-ai --bridge-status   # JSON link state per harness
./target/release/orquestra-ai --bridge-deploy   # back up + fuse every harness
```

`npm run build && cargo build --release --manifest-path src-tauri/Cargo.toml`
produces `target/release/orquestra-ai` at the repo root: this is a Cargo
workspace, so artifacts go to the workspace target directory, not
`src-tauri/target/`. `~/.local/bin/orquestra` is a symlink to it, so
`orquestra &` launches the control plane from anywhere.

## Storage layout

| Path                                | Purpose                                                        |
| :---------------------------------- | :------------------------------------------------------------- |
| `~/.orquestra/skills-library`       | Master vault. Permanent source of truth for every skill.        |
| `~/.orquestra/active/global`        | Universal baseline projection.                                  |
| `~/.orquestra/active/<harness>`     | Per-harness overlay projection (`antigravity`, `codex`, `claude`, `freebuff`, `opencode`, `kilo`, `copilot`, `hermes`, `grok`). |
| `~/.orquestra/AGENTS.md`            | Master agent instructions edited in the AGENTS.md tab.          |
| `~/.orquestra/backups/pre-fusion-<timestamp>` | Full copy of everything the bridge replaced, per harness. |

The layout is created on startup. An empty vault is seeded once from
`/mnt/amnesic/temporary_skills` when that directory exists. Disabling a skill
only removes symlinks; vault contents are never deleted.

### Inheritance

- Enabling a skill in **Global** creates its symlink in `active/global` and
  propagates it into every harness projection. Harness tabs show the skill as
  ON with an **Inherited from Global** badge and a locked toggle.
- Disabling it in Global removes the baseline and all propagated links.
- With Global off, each harness tab can project a skill independently by
  toggling it in that scope. A harness-wide disable never touches inherited
  skills.

## Harness bridge

The footer's **Harness Bridge** control runs `deploy_harness_bridge`. It is
idempotent, never deletes data, and always backs up first.

| Harness       | Skills link(s)                                             | Instructions                    | Config |
| :------------ | :--------------------------------------------------------- | :------------------------------ | :----- |
| Antigravity   | `~/.gemini/config/skills`                                  | `~/.gemini/config/AGENTS.md`     | —      |
| Codex         | `~/.codex/skills` (`.system` preserved)                    | `~/.codex/AGENTS.md`             | —      |
| Claude Code   | `~/.claude/skills`                                         | `~/.claude/CLAUDE.md`            | —      |
| Freebuff      | `~/.freebuff/skills`, `~/.config/manicode/skills`          | `~/.config/manicode/AGENTS.md`   | —      |
| OpenCode      | projection via config                                      | —                               | `~/.config/opencode/opencode.jsonc` |
| Kilo Code     | projection via config                                      | —                               | `~/.config/kilo/kilo.jsonc` |
| Copilot CLI   | `~/.copilot/skills`                                        | —                               | —      |
| Hermes Agent  | `~/.hermes/skills` (`.*` curator state preserved)          | —                               | —      |
| xAI Grok      | `~/.grok/skills`                                           | —                               | —      |

Each skills link becomes a symlink to `~/.orquestra/active/<harness>`; each
instruction file becomes a symlink to `~/.orquestra/AGENTS.md`; JSONC configs
gain `skills.paths` (`json5`-parsed, comments and other keys preserved).

Fusion merges rather than resets:

1. Skill folders that exist only inside a harness are **adopted** into the vault.
2. Skills the harness already had visible (including Hermes' nested category
   folders) are linked into the new projection, so nothing disappears on day one.
3. Harness-owned state (`codex/skills/.system`, Hermes `.*` files) moves into
   the projection.
4. Originals are moved to `~/.orquestra/backups/pre-fusion-<timestamp>/<harness>/`.
5. On the first run, non-empty harness instruction files are appended to the
   pristine master `AGENTS.md` under `## Imported from <Harness>`.

## IPC surface

| Command                              | Purpose                                                              |
| :----------------------------------- | :------------------------------------------------------------------- |
| `core_status`                        | Core version/platform, used by the footer bridge indicator.          |
| `list_skills(harnessId?)`            | Scan the vault, parse `SKILL.md`, evaluate the selected scope.       |
| `set_skill_state(id, harnessId, enabled)` | Atomic symlink update; Global propagates to all harnesses.       |
| `batch_set_all(harnessId, enabled)`  | Project or un-project every skill in one scope.                      |
| `harness_summary`                    | Active counters for Global and each harness sidebar tab.             |
| `read_master_agents_md`              | Read `~/.orquestra/AGENTS.md`, creating the default template.        |
| `write_master_agents_md(content)`    | Atomically replace the master instruction file.                      |
| `get_bridge_status`                  | Link state per harness, used by the footer indicator and modal.      |
| `deploy_harness_bridge`              | Back up, adopt, fuse, and report; returns a `BridgeReport`.          |

Token weight is estimated as `character_count / 3.8`. Harness IDs are validated
against a fixed allowlist and skill IDs are guarded against path traversal.

## Layout

```
src/
  App.tsx                     Scope loading, metrics, batch actions, views
  components/Sidebar.tsx      Global + harness navigation with live counters
  components/SkillCard.tsx    Live skill tile, inheritance badge, toggle
  components/BridgeModal.tsx  Link plan, backup warning, deploy confirmation
  components/StatusFooter.tsx System status + live harness bridge indicator
  components/AgentsEditor.tsx Line-numbered markdown editor + save controls
  components/Header.tsx       Brand, search, active badge, refresh, editor mode
  lib/harnesses.ts            Scope definitions, icons, summary type
  lib/skills.ts               `SkillItem` type and category icon mapping
  lib/tauri.ts                Typed IPC wrappers and `core_status` hook
  lib/bridge.ts               Bridge status/report types and IPC wrappers
src-tauri/
  src/lib.rs                  Tauri setup, IPC commands, storage bootstrap
  src/skills.rs               Vault scan, projections, inheritance, AGENTS.md
  src/bridge.rs               Backup, fusion, adoption, config merge, CLI output
  tauri.conf.json             1080x720 centered window, #09090b background
```

## Known limits

- A plain browser preview has no Rust core, so it shows a "Desktop bridge
  unavailable" panel; run the Tauri app to scan the vault or edit AGENTS.md.
- **Save & Deploy** writes the master `~/.orquestra/AGENTS.md`; harnesses linked
  to it pick up the change instantly, but an unlinked harness keeps its own file
  until the bridge is deployed.
- Kilo/OpenCode path injection rewrites the config through `serde_json` when a
  `skills` block already exists, which drops JSONC comments in that one case.
  Configs without a `skills` block keep their comments verbatim.
- No filesystem watcher yet: external edits to the vault need a refresh.
