# Project-Level Agent Instructions — OrquestraAI

> Local rules specific to this repository. Inherits global principles from `~/.orquestra/AGENTS.md`.

## 1. Modular Architecture (Anti-Monolith Rule)
- **Hard Line Cap:** Keep all files strictly under **300-400 lines**.
- **No Monoliths:** Never create single-file features. Decompose immediately into focused submodules:
  - Rust: Dedicated directories with `mod.rs`, separate `catalog.rs`, `serializers.rs`, `deploy.rs`, `tests.rs`.
  - React: Dedicated view components, sub-cards, sliding drawers, and custom hooks under `src/hooks/`.

## 2. Production Release Build & Root Workspace
- **Root Cargo Workspace:** The repository root is a Cargo workspace targeting `src-tauri`. Commands like `rtk cargo test` or `rtk cargo check` can be run directly from the repo root or inside `src-tauri`.
- **Automated Guardrail in `build.rs`:** Release builds automatically verify that `dist/index.html` exists. If missing, `build.rs` automatically triggers `npm run build` or halts with a clear error to prevent "Connection refused" binaries.
- **Unified Build Commands:**
  - Fast Dev Build (embedded UI): `npm run build:fast`
  - Production Release: `npm run build:release` (or `rtk npx tauri build --no-bundle`)
  - Full Test Suite (Node + Rust): `npm run test:all` (or `rtk cargo test`)
- **Verification Pipeline:**
  1. Build using `npm run build:release`
  2. Verify the release executable (`~/.local/bin/orquestra` symlink target).
     This is a Cargo workspace, so artifacts live in the root `target/`, not
     `src-tauri/target/` (which no longer exists):
     ```bash
     ./target/release/orquestra-ai --mcp-status
     ./target/release/orquestra-ai --bridge-status
     ```

## 3. Terminal & Token Efficiency (RTK Mandatory)
- Prefix terminal commands with `rtk` to avoid context bloat:
  - Use `rtk cargo check`, `rtk cargo test`, `rtk rg`, `rtk git status`.

## 4. IPC Contract & Safety
- **Zero Breaking Changes:** Never alter Tauri command signatures or serialized field names without corresponding frontend updates and regression contract tests.
- **Atomic Operations:** Always preserve non-MCP/non-managed configuration keys across harness targets.

## 5. Process Execution & Polling Protocol (No Blind Sleeps)
- **Forbidden:** NEVER use static blind sleep commands (e.g. `sleep 60`, `sleep 120`, `sleep 150`) to wait for processes, background builds or test commands to finish.
- **Synchronous Execution:** Run verification commands (`rtk cargo test`, `rtk npm test`, `rtk cargo check`) directly in the foreground.
- **Event-Driven Waiting:** If a background process is strictly necessary, wait for it event-driven via `wait $PID` or `tail --pid=$PID -f /dev/null`, or use tight polling intervals with short delays (`while kill -0 $PID 2>/dev/null; do sleep 2; done`). Never block turn execution on static sleep.

