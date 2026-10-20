# Changelog

All notable changes to **OrquestraAI** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- **Official Autonomous Code Audit Skill (`orquestra-autonomous-audit` / `autonomous-audit`):** Time-boxed autonomous code audit and hardening shift protocol across the multi-harness fleet, supporting dynamic durations ($X$ minutes), 4 universal scopes (Security, Performance, Logic, DX), atomic reproduction tests, formal negative passes, and optional Idea Vault / Agent Board lifecycle.

### Fixed
- **Official Store Resilience & Error Masking:**
  - Separated store catalog load errors from row action errors (install/enable), displaying action failures in an inline dismissible banner without unmounting the store cards.
  - Corrected error extraction in `OfficialStore.tsx` to preserve Tauri string error rejections instead of masking them as "Unexpected error".
  - Cleared stale or dangling symlinks at target destination in `import_dir`, preventing collisions when importing skills into the vault.

### Security & Hardening
- **Comprehensive Path & Symlink Traversal Protection Across Fleet Adapters:**
  - Hardened `delete_session`, `read_transcript`, and artifact sizing across all 9 session adapters (Antigravity, Claude Code, Codex, Copilot CLI, Freebuff/Manicode, Grok, Hermes Agent, OpenCode, Kilo).
  - Enforced strict single-segment validation on session IDs before path joining (`single_segment` rejecting `/`, `\`, `.`, `..` and empty strings), eliminating arbitrary file deletion through directory symlinks.
  - Replaced naive `is_dir()` and `exists()` checks with `symlink_metadata()` and `real_dir()` guards to prevent following symlinks outside harness stores.
  - Hardened Bridge, Skills, MCP, and Presets engines against symlinked projections, vault entries, `.git` roots, and configuration stores.
- **Timestamp & Dialect Sanitization:**
  - Added strict validation in `sessions/adapters/time.rs` rejecting impossible calendar dates (e.g. Feb 31, leap-year anomalies), invalid UTC offsets (`>14:00`, `< -12:00`, non-digit offset characters), and trailing garbage strings.
- **Desktop & Updater Hardening:**
  - Guarded launcher creation and `.desktop` writes against directory symlinks.
  - Replaced `cmd.exe` URL execution on Windows with direct platform opening for HTTP/HTTPS links.
  - Enforced confirmed release tag verification in in-app updater download routines.

### Performance
- **Bounded Session Log Parsing:**
  - Enforced a 32 MiB size ceiling (`MAX_JSONL_BYTES`) on Freebuff/Manicode `list_sessions`, preventing memory bloat and UI freezes on oversized `chat-messages.json` files.
  - Prevented cyclical and out-of-tree walks in directory size calculations (`dir_size`).

### Reliability & Logic
- **Maintenance Lock Validation:**
  - Hardened lock detection in Codex (`.sqlite-maintenance.lock`) and Copilot (`.session-operation-locks`) to require real regular files, eliminating denial-of-service locks caused by dangling or external symlinks.
  - Corrected ISO timezone offset sign handling during timestamp normalization.
- **Updater Version Ordering:**
  - Improved semver comparison to rank pre-releases strictly behind corresponding stable releases.

### Refactored (Anti-Monolith File Cap)
- Decomposed modules approaching the 400-line project limit into focused submodules:
  - Extracted `freebuff_tests.rs` from `freebuff.rs` and `subpath_tests.rs` from `subpath.rs`.
  - Split MCP catalog mutations into `mcp/catalog/mutate.rs` and secrets into `mcp/secrets_tests.rs`.
  - Extracted fusion IO into `bridge/fuse/fuse_io.rs` and secondary bridge tests into `bridge/tests/tests/secondary.rs`.
  - Modularized session adapter ID escape tests into `sessions/adapters/tests/id_escape.rs`.

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
