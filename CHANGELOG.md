# Changelog

All notable changes to **OrquestraAI** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- **Sanitized Freebuff Desktop Parsing:** Filter out internal `reasoning` and execution `tool` blocks from conversation turns, ensuring clean text-only bubbles in the chat drawer.
- **Branded Install Pipeline:** Support for `curl -fsSL https://internetdozero.com.br/orquestra/install.sh | bash` with Cloudflare Edge Function redirect.
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
- **Thermal & Profile Optimization:** Tuned `[profile.release]` with `codegen-units = 4` and disabled heavy LTO for fast builds.
- **Documentation Overhaul:** English and Portuguese (`README.pt-BR.md`) READMEs with MIT License and prominent Quick Install section.
- **Cargo Workspaces:** Relocated Cargo build artifacts to root `target/` and enforced 300-400 line anti-monolith limit across all adapters.

---

## [0.1.0-alpha.1] - 2026-10-18

### Initial Alpha Launch
- Official first public alpha release of the OrquestraAI desktop control plane and headless CLI.
- Unified Harness Bridge supporting skills projection and master `AGENTS.md` synchronization.
- Cross-harness MCP server management and mission presets.
- Janitor System & Memory Guard for orphan process sweeping and RAM preservation.
