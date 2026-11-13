# OrquestraAI

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Status: Alpha](https://img.shields.io/badge/Status-Alpha-orange.svg)](CHANGELOG.md)
[![Platforms](https://img.shields.io/badge/Platforms-Linux%20%7C%20macOS%20%7C%20Windows-emerald.svg)](https://github.com/internetdozero/OrquestraAI/releases/latest)

[Leia em Português](README.pt-BR.md)

OrquestraAI is a desktop app and CLI for managing several AI coding agents from one place. It keeps skills, MCP servers, instruction files, sessions and credentials consistent across Claude Code, OpenAI Codex, xAI Grok, Antigravity (AGY), GitHub Copilot CLI, Hermes Agent, OpenCode and Kilo Code.

> [!WARNING]
> **Alpha software.** Config formats, IPC commands and CLI output may change between releases. Pin automation to a tagged release.

## Features

- **Skills vault:** one library in `~/.orquestra/skills-library`, projected into each harness through symlinks or native config entries.
- **MCP registry:** enable MCP servers globally or per harness. Existing Gemini/Antigravity definitions are adopted, and official servers are installed into a private environment under `~/.orquestra/mcp-servers/`.
- **Unified instructions:** one master `AGENTS.md` linked to each harness's instruction path.
- **Sessions:** browse, search, export, pin and purge conversation histories from every supported harness.
- **Credentials vault:** API keys encrypted at rest with AES-256-GCM. The encryption key lives in the OS keyring. Agents can use keys through a blind HTTP/exec proxy without seeing them, and keys copied in the UI are cleared from the clipboard after 20 seconds.
- **Idea Vault and blackboard:** a task list and shared notes for coordinating agents.
- **Janitor:** watches memory and temporary files and cleans up orphaned agent processes.
- **Signed updates:** the in-app updater and the installer only accept releases whose checksums were signed for that exact version.
- **Portable runtime:** external tools are resolved through the login shell's `PATH` as well as the process environment, so `nvm`, `mise`, Homebrew and `~/.local/bin` installs work when the app is launched from the menu. Anything missing is reported with the exact install command.

## Install

Published binaries: **Linux x86_64**, **Linux aarch64**, **macOS Apple Silicon**, **macOS Intel** and **Windows x86_64** (an NSIS installer plus the bare executable the in-app updater replaces).

```bash
curl -fsSL https://orquestra.internetdozero.com.br/install.sh | bash
```

The installer:

1. Resolves the latest GitHub release and downloads the binary, `SHA256SUMS.txt` and `SHA256SUMS.txt.minisig`.
2. Verifies the minisign signature with the embedded release key **before** trusting any checksum. This needs [`minisign`](https://jedisct1.github.io/minisign/) or Python 3 with PyNaCl (`python3-nacl` on Debian/Ubuntu). Installation stops if neither is available or the signature is invalid.
3. Checks the binary's SHA-256 and installs it to `~/.local/bin/orquestra`.
4. On Linux, checks the binary's glibc symbol versions and the WebKitGTK/libsoup libraries, then the runtime tools (`xdg-utils` or `gio`, `git`, `python3` with `venv`), and installs what is missing through the package manager using `sudo`. Pass `--no-auto-deps` to skip the install step and manage them yourself.

Distribution support depends on glibc: the release is built on Ubuntu 22.04, so Debian 11, Ubuntu 20.04 and RHEL/Rocky 8 stop at the loader. A Flatpak bundle built from the same binary runs there. See [docs/install.md](docs/install.md) for the version table and the Flatpak channel.

To diagnose an existing Linux install without downloading again:

```bash
curl -fsSL https://orquestra.internetdozero.com.br/install.sh | sh -s -- --check-deps
```

Binaries and release notes are on the [Releases page](https://github.com/internetdozero/OrquestraAI/releases). Changes are listed in the [CHANGELOG](CHANGELOG.md).

## CLI

The same binary runs headless subcommands. Add `--json` for machine-readable output.

```bash
orquestra sessions list --harness claude
orquestra skills list --scope claude
orquestra mcp status
orquestra vault list
orquestra idea list --project MyProject
```

| Command              | Covers                                                                      | Guide                                        |
| :------------------- | :-------------------------------------------------------------------------- | :------------------------------------------- |
| `orquestra sessions` | list, inspect, search, export, pin, purge, retention policy, watch          | [docs/cli/sessions.md](docs/cli/sessions.md) |
| `orquestra skills`   | list, info, enable/disable, create, import (local or git), export, validate | [docs/cli/skills.md](docs/cli/skills.md)     |
| `orquestra mcp`      | list, status, add/remove, enable/disable, deploy, list and call tools       | [docs/cli/mcp.md](docs/cli/mcp.md)           |
| `orquestra vault`    | credentials, key pools, rotation, provider tests, blind proxy, audit        | [docs/cli/vault.md](docs/cli/vault.md)       |
| `orquestra idea`     | idea lifecycle, search, metrics, blackboard notes                           | [docs/cli/ideas.md](docs/cli/ideas.md)       |

`orquestra --help` lists the bridge and MCP shortcuts (`--bridge-status`, `--bridge-deploy`, `--mcp-status`, `--mcp-set`, `--mcp-deploy`, `--preset`).

## Supported harnesses

| Harness            | Skills                    | Instructions                 | Sessions / config                   |
| :----------------- | :------------------------ | :--------------------------- | :---------------------------------- |
| Antigravity (AGY)  | `~/.gemini/config/skills` | `~/.gemini/config/AGENTS.md` | transcript logs, `mcp_config.json`  |
| OpenAI Codex       | `~/.codex/skills`         | `~/.codex/AGENTS.md`         | `thread_history_1.sqlite`, rollouts |
| Claude Code        | `~/.claude/skills`        | `~/.claude/CLAUDE.md`        | `projects/<slug>/*.jsonl`           |
| OpenCode           | `skills.paths` in config  | native                       | `opencode.jsonc`, `opencode.db`     |
| Kilo Code          | `skills.paths` in config  | native                       | `kilo.jsonc`, `kilo.db`             |
| GitHub Copilot CLI | `~/.copilot/skills`       | native                       | `session-store.db`                  |
| Hermes Agent       | `~/.hermes/skills`        | native                       | `state.db`                          |
| xAI Grok           | `~/.grok/skills`          | native                       | `chat_history.jsonl`                |

Only user-level files are managed. Workspace `.agents` folders are not imported.

## Development

Requirements: Node.js 22+, stable Rust and, on Debian/Ubuntu:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

```bash
git clone https://github.com/internetdozero/OrquestraAI.git
cd OrquestraAI
npm install
npm run dev:tauri        # desktop app in development mode
```

Checks run by CI:

```bash
npm test                 # Node unit and contract tests
npm run lint && npm run format:check
cargo fmt --all -- --check
cargo test --workspace
```

Stack: Tauri v2 and a Rust workspace (`crates/`: core, harness, vault, bridge, sessions) with a React 19 + TypeScript + Vite frontend styled with Tailwind CSS v4. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/IPC_API.md](docs/IPC_API.md), [docs/vault.md](docs/vault.md) and [docs/security.md](docs/security.md).

Official skills and MCP servers live in [internetdozero/orquestra-extensions](https://github.com/internetdozero/orquestra-extensions).

## License

[MIT](LICENSE)
