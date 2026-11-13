# Changelog

All notable changes to **OrquestraAI** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0-alpha.9] - 2026-11-13

> **Freebuff support removed.** Freebuff now keeps skills, MCPs and global
> instructions in its cloud account, so the folder-based harness integration no
> longer had any effect. The harness, its session adapters, its MCP target, its
> instruction/skill links and the cloud-session badge are gone. The remaining
> eight harnesses keep full skills, MCP and `AGENTS.md` support, and existing
> `~/.agents` content is left untouched — it is no longer adopted or
> quarantined.

### Added

- **Capability Probe:** Settings → About now reports every external program the app shells out to — `git`, `python` (with `venv`/`ensurepip`), `node` and the desktop opener — with the exact install command for the platform. It also names the Flatpak sandbox, where host binaries are invisible, instead of offering actions that cannot work.
- **Flatpak Channel:** The release publishes `OrquestraAI-x86_64.flatpak`, wrapping the Linux binary with the GNOME 47 runtime so Debian 11, Ubuntu 20.04, RHEL/Rocky 8 and Amazon Linux 2 can run it despite their older glibc and WebKitGTK 4.0; the installer names the exact Flatpak command when it refuses a binary the host cannot load. The manifest grants `org.freedesktop.secrets` and `xdg-run/keyring`, and the build installs and smoke-tests the bundle before publishing it.
- **Bridge Backup Retention:** Keep the three newest `pre-fusion-*` recovery directories after a fully successful bridge; preserve failed-run and unrelated backups.
- **Wayland Software Rendering:** `ORQUESTRA_WEBKIT_SOFTWARE=1` sets `WEBKIT_DISABLE_DMABUF_RENDERER` before the window is created, for NVIDIA/Wayland sessions where WebKitGTK opens a blank window. `GDK_BACKEND` is still never overridden.
- **Bundle Guard:** `npm run check:bundle` runs after every production build and fails if developer fixtures or their absolute paths appear in `dist/`.

### Changed

- **Portable Executable Resolution:** A single resolver in `orquestra-core` searches the process `PATH` and then the login shell's `PATH` plus `~/.local/bin`, `~/.cargo/bin`, mise/asdf shims and Homebrew, and reads the Windows user `PATH` from `HKCU\Environment`. The desktop shell and `vault exec` share it, so tools installed by nvm, mise or Homebrew work when the app is launched from the menu.
- **Linux Launcher:** The generated `.desktop` entry uses the absolute executable path (the `.AppImage` file when running from one) with Desktop Entry quoting instead of a bare `orquestra` command. Inside Flatpak the app no longer writes a launcher that would shadow the exported one.
- **Open Links and Folders:** Falls back across `gio open`, `xdg-open`, `kde-open6/5`, `sensible-browser`/`x-www-browser` and the common file managers instead of requiring `xdg-utils`.
- **Installer Runtime Tools:** `install.sh` and `--check-deps` now diagnose and (unless `--no-auto-deps`) install `xdg-utils` or `gio`, `git` and `python3` with `venv`, naming the package per distribution.
- **Debian Package Dependencies:** The `.deb` declares WebKitGTK 4.1, GTK 3, libsoup 3, libayatana-appindicator, librsvg and `xdg-utils`, so `dpkg -i` resolves them instead of leaving a binary that cannot start.
- **Faster Views:** Ideas and sessions are served from TTL caches with stale-while-revalidate, the remaining caches expire instead of living forever, harness mounts are cached and views prefetch on hover. `src/lib/perf.ts` records boot and view-switch timings.
- **Shared Agents Skills:** `~/.agents/skills` is tracked as a secondary skills source for Codex and Grok, so skills installed there are not silently ignored.

### Removed

- **Freebuff Harness Support:** The harness, session adapter, MCP target, instruction/skill links and cloud-session badge were removed (63 files). `~/.agents` stays on disk but is no longer adopted, quarantined or projected; Codex and Grok read its skills as a secondary source only.

### Fixed

- **Keyring Probe and Fallback:** A live D-Bus session no longer implies a live Secret Service. The vault performs one bounded probe (3 s) and falls back to the `0600` key file with a reported reason instead of blocking on provider activation.
- **Python MCP Installs:** `python3 -m venv` is checked before cloning, so Debian/Ubuntu without `python3-venv` get an actionable message instead of `ensurepip is not available`; the Microsoft Store `python` alias is refused on Windows.
- **`skills export --tar`:** The archive is written in-process with `tar` + `flate2`, matching the backup path, so it works on Windows builds without `tar.exe` and on minimal Linux images.
- **macOS First Launch:** Probing `git` no longer opens the Xcode Command Line Tools dialog; the shim at `/usr/bin/git` is reported as missing (with `xcode-select --install`) when the tools are absent.
- **Desktop Cache Refresh:** Tries `kbuildsycoca6` and `kbuildsycoca5`, only runs installed helpers, and logs one warning naming the command and the consequence instead of discarding every error.
- **Developer Fixtures Out of Production:** Mock sessions, ideas, MCP catalogs and their `/home/operator` paths moved behind a dev-only dynamic import; the production bundle no longer emits them.
- **Windows App Identity:** The Start Menu shortcut now carries `System.AppUserModel.ID`, so the window, the tray icon and the Start Menu entry group as one app instead of by executable path, and a shortcut written by an older build is refreshed once instead of being treated as current forever.
- **Linux Compatibility Gate:** The `debian:11` and `rockylinux:8` jobs assert the diagnosis (a measurable glibc requirement and a report that names the Flatpak) instead of failing on the documented baseline, so only an unmeasurable requirement turns the pipeline red.

### Security

- The Flatpak sandbox now reaches the OS keyring (`--talk-name=org.freedesktop.secrets`, `--filesystem=xdg-run/keyring`); without a provider the vault degrades explicitly instead of silently.
- `vault exec` validates the bare allowlisted name and the per-binary subcommand policy **before** resolving it, so IPC still cannot supply a path and the login-shell lookup only chooses among binaries with an allowed name.

## [0.1.0-alpha.8] - 2026-11-07

### Added

- **Chrome App Icon:** The desktop icon is the chrome symbol from the brand kit, regenerated across every platform asset (PNG set, `.icns`, `.ico`).
- **Windows Artifact:** A `windows-latest` build produces an NSIS installer, `install.ps1` installs and updates it, and the updater accepts the Windows asset instead of refusing the platform.
- **macOS App Bundle:** `tauri build --bundles app,dmg` ships a real bundle, with codesign and notarization gated on repository secrets, so Finder no longer blocks the download.
- **Linux Compatibility Gate:** `debian:11` and `rockylinux:8` jobs run the built binary against their own glibc, and `scripts/check-glibc.sh` reports missing symbol versions before a release goes out.
- **Debian Package:** The Linux build publishes a `.deb` alongside the tarball.
- **RTK PATH Guidance:** The RTK installer reports the exact `export`/`setx` line when its target directory is not on `PATH`.
- **Multiplatform Path Shortening:** `shortenPath` supports both Windows (`C:\Users\...`) and Unix (`/home/...`) paths seamlessly in the frontend UI and Janitor modals, backed by unit tests.
- **Ideas CLI Filter Discovery:** Explicit `--status all` documentation and test coverage for `orquestra idea list`, surfacing the default `open` filter behavior.

### Fixed

- **Windows Instruction Links:** A junction describes only a directory, so the `AGENTS.md` links never existed on Windows; fusion now creates a hard link there, recognises it, and re-points it after the instruction import republishes the master.
- **Release Build Portability on Windows:** `build.rs` resolves `npm` via `PATHEXT` to avoid failed builds when `npm` is invoked as `npm.cmd` on Windows.
- **Tray Fallback Window Close:** App exits gracefully on window close when the desktop environment or OS lacks system tray support.
- **Ideas CLI Unknown Subcommand Handling:** Fails with a clear usage error on unknown subcommands instead of silent non-zero termination, keeping `--help` and dispatch in sync.
- **SQLite Test Concurrency:** Added shared mutex lock helper to CLI idea status filter tests, preventing SQLite database locks during multi-threaded test runs.

## [0.1.0-alpha.7] - 2026-11-06

### Added

- **Freebuff Cloud Sessions Handling & Badging:** Detects Freebuff Desktop sessions processed remotely in Codebuff Cloud (`execution_target = 'cloud'`). Sessions pending local sync display an informative cloud badge (`Nuvem (não sincronizada)` / `Cloud (unsynced)`) and tooltip in the UI, and inspecting their transcript returns clear diagnostic guidance directing the user to open them in Freebuff Desktop to sync messages.
- **Granular Native CLI Subcommands:** Comprehensive CLI families matching the full desktop and agent capabilities:
  - `orquestra mcp`: `list`, `status`, `inspect`, `add`, `remove`, `enable`, `disable`, `deploy`, `targets`, `test-connection`, `tools-list`, and dynamic `call-tool` execution with table, wide, and JSON outputs.
  - `orquestra ideas` & `board`: Granular task lifecycle, status transitions, search, metrics, and blackboard noticeboard operations.
  - `orquestra sessions`: `list`, `inspect`, `export` (full Markdown/JSON transcripts), `pin`/`unpin`, `purge` (with dry-run estimations), `delete`, `policy`, and live activity `watch`.
  - `orquestra skills`: `list`, `info`, `enable`, `disable`, `batch` toggles, `summary`, `validate`, `create`, `import` (local folders and git URLs with `#subpath`), `export`, `content`, and safe `delete`.
  - `orquestra vault`: Granular blind credential operations, multi-key pool management (`add-key`, `remove-key`, `set-primary`), pointer rotation (`rotate`), provider verification (`test`), presets discovery (`presets`), and security auditing (`audit`).
- **Blind Vault Key Pinning & Selection:** Support selecting specific key numbers or tags when dispatching blind HTTP and exec requests, preserving active default key pointers.
- **Credentials Vault Import:** Direct import and resealing of vault credentials from JSON files with AES-256-GCM encryption.
- **Timed Clipboard Clearing:** Automatic timed clipboard clearing (60s) when revealing and copying credential keys in the UI.
- **RTK Usage Skill & Integration:** Official RTK integration skill recommending token-condensing wrappers for agent CLI interactions, with automatic binary detection in the store.
- Installer automatically detects package manager and auto-installs missing Linux runtime dependencies (WebKitGTK, GTK3, libsoup, libayatana-appindicator) using sudo/root, with `--no-auto-deps` opt-out.
- Localized guidance for MCP servers that do not provide a description, directing users to inspect the server's tools and capabilities.
- Active MCP counts in the Global and harness scope tabs; harness counts include servers inherited from Global.
- Store installation feedback for skills and MCPs, with immediate loading state and readable clone/install failures.

### Changed

- Reorganized Freebuff Desktop session adapter into a dedicated `query.rs` module adhering to the anti-monolith file size cap.

### Fixed

- **Cleartext session-id logging:** CLI human and error messages for session delete/pin/unpin/export/inspect/handoff/watch no longer interpolate raw session identifiers into stdout/stderr (CodeQL `rust/cleartext-logging` alerts #12 and #13). Structured `--json` payloads still return `sessionId` for operators who requested that session.
- **Release runner:** macOS Apple Silicon builds move from retired `macos-14` to `macos-15` after GitHub's 2026-11-02 image retirement.
- **minimatch:** force development transitive `minimatch` to `11.0.1` via npm `overrides` to close Dependabot alert #4 (globparts amplification DoS); production runtime is unaffected.
- Kept scoped ElevenLabs keys active on 401 response during credential verification.
- Corrected provider connection checks and latency measurements for blind vault credentials.
- Freebuff bridge detection now uses its global `~/.agents` root and ignores optional skill or instruction paths whose parent directories are absent.
- Existing MCPs are adopted from the global Freebuff and Gemini configuration files, including URL-based servers such as Exa and command-based servers such as REA. Activating an official MCP installs it into Orquestra's private environment, and the install work no longer blocks the UI.
- MCP Store errors now surface the installation failure details instead of leaving a server inactive without explanation.
- Harness bridge deployment keeps skill originals in Orquestra's vault, backs up replaced native files, and leaves harness discovery paths linked to the managed projection. Gemini built-in skill paths are archived so they cannot bypass that projection.

## [0.1.0-alpha.6] - 2026-11-04

### Added

- Installer checks Linux runtime libraries before replacing an installation and provides `--check-deps` to diagnose a broken launch later without reinstalling.
- macOS health reporting for system memory, process memory, and process activity, compiled on a dedicated macOS CI runner.
- Secure removal of plaintext migration backups from Credentials, with verification, explicit confirmation, and a key-rotation advisory.
- Tag-pinned installer delivery through the Cloudflare Worker, with ETag caching and a deploy-time parity check.
- Windows and macOS compile checks, plus an automated Rust file-length guard in CI.

### Changed

- Idea Vault clears an idea's `NEW` badge after it is opened or acted on, and persists that state across refreshes.
- The installer and updater support Linux, macOS Apple Silicon, and macOS Intel assets; macOS installs skip Linux desktop integration.
- Rust code is organized into dedicated core, harness, vault, bridge, and sessions workspace crates.
- Session readers and purge tests are split into focused modules; macOS process inspection uses native APIs instead of spawning `ps`.

### Fixed

- **Installer release-tag resolution:** the curl installer now reads the release tag from the `/releases/latest` redirect instead of the final asset URL (GitHub now serves assets from `release-assets.githubusercontent.com`), fixing "Security Error: could not safely resolve the release tag". This release is signed with the version-bound trusted comment the installer requires.
- Vault recovery falls back to verified migration backups without destroying the original files, and keeps fallback-key reads available if keyring promotion fails.
- Windows home-directory lookup falls back to `USERPROFILE`; environment-mutating tests are serialized to avoid cross-test races.
- Janitor health checks fail closed when socket liveness cannot be established.
- Session imports recognize text blocks without exposing tool results, and Codex remote headers serialize correctly.
- Azure CLI policy blocks access-token generation even when global options precede the operation.

### Security

- Installer verifies a release-bound signature before trusting checksums; `--allow-unsigned-legacy` opts into checksum-only installs for older releases.
- Vault hardening adds parser-parity SSRF checks, tighter endpoint allowlists, strict per-binary command policies, and safer key failover and health reporting.
- Updater signatures are bound to the selected version and reject downgrade attempts.
- Upgrades from before `alpha.5` may leave plaintext `.migrate-bak` files. Verify credentials, purge remaining backups in Credentials, and rotate keys stored before encryption.

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
