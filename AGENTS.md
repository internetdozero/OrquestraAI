# Project-Level Agent Instructions — OrquestraAI

> Local rules specific to this repository. Inherits global principles from `~/.orquestra/AGENTS.md`.

## 1. Modular Architecture (Anti-Monolith Rule)
- **Hard Line Cap:** Keep all files strictly under **300-400 lines**.
- **No Monoliths:** Never create single-file features. Decompose immediately into focused submodules:
  - Rust: Dedicated directories with `mod.rs`, separate `catalog.rs`, `serializers.rs`, `deploy.rs`, `tests.rs`.
  - React: Dedicated view components, sub-cards, sliding drawers, and custom hooks under `src/hooks/`.

## 2. Mandatory Production Release Build (CRITICAL)
- **FORBIDDEN:** NEVER use direct `cargo build --release` to produce the production executable. Direct `cargo build` does NOT set Tauri release context, causing Tauri to compile with `devUrl` (`http://127.0.0.1:1420`), resulting in "Could not connect to 127.0.0.1: Connection refused" when launched without Vite.
- **MANDATORY BUILD PIPELINE:** Whenever frontend code, Rust backend code, Tauri commands (`lib.rs`), or IPC surfaces change:
  1. Rebuild using the official Tauri CLI:
     ```bash
     rtk npx tauri build --no-bundle
     ```
     *(This automatically runs `npm run build` via `beforeBuildCommand`, injects production flags, and embeds the production `../dist` static assets into the binary).*
  2. Verify the release executable (`~/.local/bin/orquestra` symlink target):
     ```bash
     ./src-tauri/target/release/orquestra-ai --mcp-status
     ./src-tauri/target/release/orquestra-ai --bridge-status
     ```
  - Never declare a feature complete relying only on `cargo check`, `cargo test`, or direct `cargo build`.

## 3. Terminal & Token Efficiency (RTK Mandatory)
- Prefix terminal commands with `rtk` to avoid context bloat:
  - Use `rtk cargo check`, `rtk cargo test`, `rtk rg`, `rtk git status`.

## 4. IPC Contract & Safety
- **Zero Breaking Changes:** Never alter Tauri command signatures or serialized field names without corresponding frontend updates and regression contract tests.
- **Atomic Operations:** Always preserve non-MCP/non-managed configuration keys across harness targets.
