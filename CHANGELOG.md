# Changelog

All notable changes to **OrquestraAI** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Security

- Plaintext migration backups (`.migrate-bak`): the at-rest encryption migration kept readable copies of the secrets in `vault.json.migrate-bak` and `mcps.json.migrate-bak` as a safety net. These files stay on disk with mode `0600`, but any secret stored before `v0.1.0-alpha.5` may have existed as plaintext.
- Recommended action: (1) confirm every service loads normally; (2) use the **"Purge plaintext backups"** button on the Credentials screen (command `vault_purge_migration_backups`, behind an explicit confirmation) to overwrite the backups with zeroes and delete them; (3) **rotate the provider API keys** stored before `alpha.5` in their dashboards (GitHub, OpenAI, Anthropic, Cloudflare, and others). Deletion is best-effort on SSDs and copy-on-write filesystems — rotation removes the residual risk.
- Purging never destroys the only readable copy: each backup is deleted only if its sealed file still opens under the current key.
- Encryption hardening: sealed envelopes are now version `2` and bind the payload domain (`vault.v1` / `mcps.v1`) as AES-GCM Additional Authenticated Data, so a `vault.json` envelope pasted into `mcps.json` (or vice versa) fails closed; `alpha.5` v1 envelopes still open and are re-sealed as v2 on load. Key resolution tries the OS keyring first with the `0600` key file as fallback (created `0600` from birth, mode/owner verified), promotes fallback bytes into the keyring on first unseal, and never mints a key as a side effect of a read. New migrations leave no plaintext behind — the backup is unlinked once the sealed file verifies — and once an install completes the migration, plaintext `vault.json` / `mcps.json` files are refused instead of re-sealed. Secrets stored before encryption existed were left in plaintext backups: users of earlier alphas should still rotate those keys.

## [0.1.0-alpha.5] - 2026-10-22

> **Note:** release `alpha.4` was immediately superseded by this version to resolve static-analysis alerts.

### Added

- **Blind Vault credential store:** API keys and secrets in `vault.json` and `mcps.json` are sealed with AES-256-GCM. Blind HTTP and exec proxy (`vault_http_request` / `vault_exec`) injects secrets in native Rust so credentials never cross JavaScript memory or LLM context.
- **Line-by-line session streaming:** `.jsonl` files and rollouts (Claude, Codex, Grok) are read on demand (`for_each_jsonl`) instead of loading whole files into RAM.
- **Virtualized transcript feed** with debounced search for large conversations.
- **Bundle splitting:** lazy-loading of secondary modals and dialogs, reducing startup time.
- **Batched projection resolution** (`projected_ids`), replacing hundreds of per-path canonicalization calls with a single disk pass.
- **Reliable single instance** via an OS-level lock (`.orquestra.lock`) with socket teardown and signal cleanup on restart.
- **Concurrent atomic writes** with unique per-thread, per-process temporary suffixes (`unique_tmp_suffix`), preventing interleaved bytes on overlapping IPC writes.
- **Vault health status dots** with a theme-independent color ramp.
- **Idea Vault upgrades:** monospace `#id` chips, a `NEW` indicator for recent tasks, Enter-key confirmation, and SWR cache persistence.
- **Animation guardrail:** automated pre-commit check (`scripts/check-card-motion.mjs`) rejecting unnecessary `transition: all` in the UI.
- **Official `orquestra-web-search` MCP** integration and registration.
- **`~/.agents/skills` as the canonical skill root**, without collisions with other harnesses.
- **Active security audit:** recursive inspection of executable scripts at install time, with UI badges before installation.
- **Legacy alias suppression:** automatic hiding of legacy folders when the canonical `orquestra-*` package is active.
- **Frontend Error Boundaries** with recovery and structured backend logging.

### Changed

- **Strict Content Security Policy:** removed `unsafe-inline` from styles.

### Fixed

- **CodeQL:** zero open alerts, via dynamic key and nonce generation (`Aes256Gcm`, `OsRng`).
- **Prettier and Cargo fmt synchronization** of the IPC contract and TypeScript typings.

### Security

- **AES-256-GCM encryption at rest** with the file key in the OS keyring (Secret Service, Keychain, Credential Manager) or a `0600` fallback key file.
- **Ed25519-signed native updater:** download and integrity verification in Rust crates (no `curl`/`wget` dependency), with detached `SHA256SUMS.txt.minisig` signatures.
- **Tauri v2 IPC capability isolation:** granular scopes restrict sensitive commands from the main window.
- **Anti-exfiltration on blind paths:** per-preset HTTPS allowlists and secret scrubbing across Base64, URL, and Hex encodings in logs and responses.

## [0.1.0-alpha.3] - 2026-10-19

### Added

- **Official Skills Suite Modernization & Canonical Alignment:**
  - Modernized the 4 official core skills (`orquestra`, `orquestra-modular-start`, `orquestra-agent-orchestrator`, `orquestra-smart-build-router`) in the official monorepo `internetdozero/orquestra-extensions`.
  - Registered `@internetdozero` UI/UX skills (`lean-screen` and `ux-best-practices`) as official first-class skills in `src-tauri/src/official.rs`.
  - Integrated Idea Vault task lifecycle (#251-#256), multi-harness fleet protocols (`session-relay`, `janitor`), `rtk` command efficiency, and Zero-Heat Cloud Builds.
  - Repaired broken symlinks in local vault (`~/.orquestra/skills-library/`) and harness projections.
  - Consolidated GitHub footprint: removed 11 redundant standalone repositories in favor of the canonical monorepo.
- **Community Skills & MCP Hub (#229):** In-app discovery catalog for community skills and MCP servers with category and tier filters (`all`, `official`, `community`), 1-click install, and instant projection into active harness scopes.
- **Static Skill Security Audit:** Recursive directory inspector flagging external executable scripts (`.sh`, `.bash`, `.py`, `.js`, `scripts/`, `bin/`) and surfacing UI security badges before installation.
- **Freebuff Desktop Deduplication Pass (#210):** Worktree resolver correlating 36-char SQLite thread UUIDs with 8-char prefix worktree directories; eliminates double-counting disk usage and reclaims dormant worktrees upon thread deletion.
- **Official Extensions Monorepo Architecture (#212):** Core support for `internetdozero/orquestra-extensions` with subpath anchor extraction (`#skills/...`, `#mcps/...`), deep monorepo folder discovery, and fallback repository routing.
- **Ideas Backlog & Notion-Style Productivity Upgrades:**
  - One-click "Copy for AI Agent" across Kanban cards, list view rows, and detail modal formatting ideas into markdown prompts ready for agent dispatch.
  - Interactive Notion-style column sorting in list view (`title`, `priority`, `category`, `updated`).
  - Collapsible Completed column by default on board load to minimize visual clutter.
  - Productivity heatmap and analytics sub-panel.
- **In-App Updater Engine:** Background release detector and 1-click update flow in Settings > About.
- **Monorepo Scaffolding Script:** Executable `scripts/scaffold-extensions-monorepo.sh` utility to bootstrap extensions monorepos with subtree support.

### Changed

- **Branding & UI Polishing:** Consolidated library tabs and MCP toolbar into a shared control row, aligned order, eliminated preset pill dead space, tightened list density, and improved hover stability.
- **Zero-Heat Cloud CI:** Refined GitHub Actions release matrix to build Linux and macOS Apple Silicon binaries cleanly without thermal throttling.

### Fixed

- **CodeQL CWE-754 Alert:** Validated dynamic method invocation on WebSocket CDP resolver in `scripts/visual/cdp.mjs`.
- **Stale Lockfile Cleanup:** Removed obsolete `src-tauri/Cargo.lock` in favor of root Cargo workspace, resolving Dependabot duplicate alert.

## [0.1.0-alpha.2] - 2026-10-19

### Added

- **Linux Desktop Integration & Auto-Launcher:** Automated creation of `.desktop` launcher and multi-resolution application icons (`16x16` up to `512x512`) on initial app startup and direct script install for Wayland and X11 docks.
- **Visual Validation Suite & Multi-Harness Fixtures (#228):** In-app fixture gallery and transcript feed validation suite for all 9 AI harnesses.
- **Branded Install Pipeline:** One-line installer hosted at `curl -fsSL https://orquestra.internetdozero.com.br/install.sh | bash` with Cloudflare Edge Function routing.
- **Fail-Closed Installer Security:** Strictly single-entry SHA-256 validation with fail-closed integrity checks in `install.sh`.
- **Zero-Dependency Structured Logging:** Lightweight `src-tauri/src/log.rs` (`log_error!`, `log_warn!`, `log_info!`) with UTC timestamps and call-site module paths, replacing raw `eprintln!` calls.
- **Automated CI Test Pipeline:** Added `.github/workflows/ci.yml` running full workspace cargo tests, clippy with `-D warnings`, npm unit tests, and TypeScript typechecking on every push and PR.
- **CodeQL Rust & Security Workflows:** Expanded `.github/workflows/codeql.yml` matrix to include Rust with WebKitGTK prerequisites and compiler interception.
- **Cross-Platform Cloud CI Matrix (#230):** Extended GitHub Actions workflow to cross-compile release binaries for Linux x86_64, macOS Apple Silicon (arm64), and macOS Intel (x86_64) with unified SHA-256 release packaging.
- **Rich Markdown Transcript Viewer:** Custom zero-dependency `MarkdownTurn` component rendering styled code fences with copy buttons, inline code, headings, lists, and quotes.
- **Transcript Sanitization:** Stripped `<user_info>` / `<rules>` environment scaffolding and `synthetic_reason` reminders in xAI Grok, and filtered internal `reasoning` and execution `tool` blocks from Freebuff Desktop turns.
- **Universal Transcript Parsers (#220):** Native transcript reader engines for all 9 AI harnesses:
  - Antigravity: unwraps `<USER_REQUEST>` prompt wrappers and maps `USER_EXPLICIT` / `MODEL`.
  - xAI Grok: recursive `chat_history.jsonl` scan with path traversal guards.
  - OpenAI Codex: reads multi-file `rollout-*.jsonl` filtering `response_item` user/assistant speech.
  - GitHub Copilot CLI: direct read-only SQLite reader for `session-store.db` turns.
  - Hermes Agent: SQLite reader for `state.db` messages filtering active rows.
  - OpenCode & Kilo: shared SQLite parts + message reader with multi-version fallback.
  - Freebuff / Manicode: project chat JSON arrays and native SQLite `desktop-v2.db`.
  - Claude Code: projects JSONL log extractor with clean Markdown sanitization.
- **Transcript Inspector Power Tools:** In-chat search with match badges, full conversation copy-to-clipboard, and structured Markdown export.
- **Zero-Heat Cloud CI:** Remote compilation workflow on GitHub Actions (`.github/workflows/build.yml`) preserving local laptop thermals.
- **Smart Build Router Skill:** Change-surface intelligence to prevent wasteful compiler runs and CPU overheating.

### Changed

- **Strict Clippy & Anti-Panic Hardening:** Enforced `#![deny(clippy::unwrap_used)]` and `#![deny(clippy::expect_used)]` in production code; eliminated bare `expect()` in `mcp/deploy.rs` and `mcp/secrets.rs` in favor of `Result` propagation.
- **Automated Dependabot Security:** Added `.github/dependabot.yml` tracking weekly updates and group management with major ignore guards for Cargo, npm, and GitHub Actions.
- **Thermal & Profile Optimization:** Tuned `[profile.release]` with `codegen-units = 4` and disabled heavy LTO for fast builds.
- **Documentation Overhaul:** English and Portuguese (`README.pt-BR.md`) READMEs with MIT License and prominent Quick Install section.
- **Cargo Workspaces:** Relocated Cargo build artifacts to root `target/` and enforced 300-400 line anti-monolith limit across all adapters.

### Fixed

- **Cross-Platform statvfs Casting:** Added explicit `u64` casting for Darwin/macOS `statvfs` struct fields (`f_bavail`, `f_bfree`, `f_bsize`) in Janitor system health monitor.

---

## [0.1.0-alpha.1] - 2026-10-18

### Initial Alpha Launch

- Official first public alpha release of the OrquestraAI desktop control plane and headless CLI.
- Unified Harness Bridge supporting skills projection and master `AGENTS.md` synchronization.
- Cross-harness MCP server management and mission presets.
- Janitor System & Memory Guard for orphan process sweeping and RAM preservation.
