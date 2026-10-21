# Project-Level Agent Instructions — OrquestraAI

> Local rules specific to this repository. Inherits global principles from `~/.orquestra/AGENTS.md`.

## 1. Modular Architecture (Anti-Monolith Rule)

- **Hard Line Cap:** Keep all files strictly under **300-400 lines**.
- **No Monoliths:** Never create single-file features. Decompose immediately into focused submodules:
  - Rust: Dedicated directories with `mod.rs`, separate `catalog.rs`, `serializers.rs`, `deploy.rs`, `tests.rs`.
  - React: Dedicated view components, sub-cards, sliding drawers, and custom hooks under `src/hooks/`.

## 2. Production Release Build & Root Workspace

- **Root Cargo Workspace:** The repo root is a Cargo workspace targeting `src-tauri`. Run checks directly from repo root.
- **Automated Guardrail in `build.rs`:** Release builds verify `dist/index.html` exists; auto-triggers `npm run build` or halts with a clear error.
- **Unified Build Commands:**
  - Fast Dev Build: `npm run build:fast`
  - Production Release: `npm run build:release` (or `rtk npx tauri build --no-bundle`)
  - Full Test Suite: `npm run test:all` (or `rtk cargo test`)
- **Verification Pipeline:**
  1. Build using `npm run build:release`
  2. Verify release executable (`~/.local/bin/orquestra` symlink):
     ```bash
     ./target/release/orquestra-ai --mcp-status
     ./target/release/orquestra-ai --bridge-status
     ```

## 3. IPC Contract & Safety

- **Zero Breaking Changes:** Never alter Tauri command signatures or serialized field names without corresponding frontend updates and regression contract tests.
- **Atomic Operations:** Always preserve non-MCP/non-managed configuration keys across harness targets.

## 4. Remote Compilation & Thermal Preservation (GitHub Actions)

- **Zero-Heat Cloud Builds:** To avoid severe CPU thermal throttling (~93 °C) on local dual-core hardware during heavy release compilation, compile remotely via GitHub Actions:
  - Workflow: `.github/workflows/build.yml`
  - Trigger: `rtk gh workflow run "Remote Build & Release" --ref main` (or pushing a tag `v*`)
  - Pull Artifact:
    ```bash
    rtk gh run download -n orquestra-linux-x86_64 && chmod +x orquestra-linux-x86_64/orquestra-linux-x86_64
    ```
- **Local Dev vs. Release:** For day-to-day code changes, use `npm run dev` or fast incremental builds (`cargo check`, `npm run build:fast`). Avoid consecutive local release builds (`npm run build:release`).
- **Targeted Checks over Blanket Builds:**
  - If changes only touch frontend or docs (`src/`, `docs/`, `*.md`), DO NOT run `cargo check`. Use `npm run typecheck` or `npm run build:fast`.
  - When verifying Rust changes locally, target the specific crate (`rtk cargo check -p orquestra-ai --lib`) rather than blanket `cargo check --workspace`.
  - Keep `jobs = 2` to avoid 100% CPU core saturation and thermal trip points.

## 5. Commit Discipline (Mandatory)

- **Every task ends in a commit:** Never hand back a dirty tree. Stage and commit all task output (code, configs, docs) once the gates pass.
- **Conventional subjects:** `<type>(<scope>): <imperative summary>`, e.g. `fix(vault): reject shell metacharacters in exec args`; short body only for a non-obvious why.
- **Scan before staging:** Grep the staged diff for secrets, tokens and signing seeds — credentials and signing material never enter the repo.
- **Push only on request:** Committing is mandatory; pushing is not. Push or open a PR only when the user explicitly asks.
