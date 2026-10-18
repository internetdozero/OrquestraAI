# OrquestraAI

<div align="center">

**Universal AI Harness Administration & Control Plane**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Status: Alpha](https://img.shields.io/badge/Status-Alpha%20(Active%20Development)-orange.svg)](https://github.com/internetdozero/OrquestraAI)
[![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20Tauri%20v2-emerald.svg)](https://github.com/internetdozero/OrquestraAI)

[**Read in Portuguese / Leia em Português (README.pt-BR.md)**](README.pt-BR.md)

</div>

---

> [!WARNING]
> **ALPHA RELEASE — ACTIVE DEVELOPMENT**
> OrquestraAI is currently in active **Alpha**. Architecture, APIs, and interfaces are evolving rapidly. While the harness bridge, skills vault, session telemetry, and MCP management are thoroughly tested and production-ready in local workflows, breaking schema migrations or structural updates may occur between alpha revisions.

---

## ⚡ Quick Install

```bash
curl -fsSL https://internetdozero.com.br/orquestra/install.sh | bash
```

---

## 🌟 What is OrquestraAI?

Modern AI engineering often involves using multiple AI coding agents and CLI harnesses concurrently: **Claude Code**, **OpenAI Codex**, **xAI Grok**, **Antigravity (AGY)**, **Freebuff / Manicode**, **GitHub Copilot CLI**, **Hermes Agent**, **OpenCode**, and **Kilo Code**.

However, each tool maintains its own isolated skill definitions, separate MCP server configurations, fragmented system instructions (`AGENTS.md` vs `CLAUDE.md`), and scattered session histories.

**OrquestraAI** solves this fragmentation with a high-performance **Tauri v2 + Rust + React** desktop application and headless CLI. It provides:
1. **Universal Skills Vault & Projections:** Maintain one central skills repository (`~/.orquestra/skills-library`) and project skills dynamically (via symlinks or native JSONC config injection) into any harness with zero disk duplication.
2. **Global & Harness-Scoped MCP Server Registry:** Enable, disable, and configure Model Context Protocol (MCP) servers globally or per individual AI harness.
3. **Master `AGENTS.md` Protocol:** Keep your system-level rules and behavioral instructions unified across all tools without manual copy-pasting.
4. **Cross-Harness Session Telemetry & Transcript Viewer:** Inspect disk footprint, turn counts, and full live conversation transcripts across all 9 harnesses with full sanitization (stripping thinking tags, extracting clean user prompts).
5. **Janitor System & Memory Guard:** Prevent OOM kills on developer laptops, sweep orphan agent processes, and clean transient caches safely.
6. **Idea Vault & Inter-Agent Blackboard:** Persistent scratchpad and communication bridge for multi-agent workflows.

---

## 🛠️ Supported Harnesses (The 9 Core Agents)

| Harness | Skills Projection Path | Instruction File | Config / Database Integration |
| :--- | :--- | :--- | :--- |
| **Antigravity (AGY)** | `~/.gemini/config/skills` | `~/.gemini/config/AGENTS.md` | Session logs (`.system_generated/logs/transcript.jsonl`) |
| **OpenAI Codex** | `~/.codex/skills` | `~/.codex/AGENTS.md` | `thread_history_1.sqlite` + multi-file rollouts |
| **Claude Code** | `~/.claude/skills` | `~/.claude/CLAUDE.md` | `projects/<slug>/*.jsonl` |
| **Freebuff / Manicode** | `~/.freebuff/skills`, `~/.config/manicode/skills` | `~/.config/manicode/AGENTS.md` | `desktop-v2.db` + project chat logs + worktrees |
| **OpenCode** | `skills.paths` config injection | Native instructions | `~/.config/opencode/opencode.jsonc` + `opencode.db` |
| **Kilo Code** | `skills.paths` config injection | Native instructions | `~/.config/kilo/kilo.jsonc` + `kilo.db` |
| **GitHub Copilot CLI** | `~/.copilot/skills` | Native instructions | `session-store.db` (turns schema) |
| **Hermes Agent** | `~/.hermes/skills` | Native instructions | `state.db` (messages schema) |
| **xAI Grok** | `~/.grok/skills` | Native instructions | `chat_history.jsonl` + session index SQLite |

---

## 🏗️ Architecture & Stack

- **Shell / Native Core:** [Tauri v2](https://v2.tauri.app/) (Rust 2021, native Linux GTK/WebKit)
- **Frontend:** React 19 + TypeScript + Vite 8
- **Styling:** Tailwind CSS v4 (Custom Dark Zinc/Slate palette, dynamic accent themes)
- **State & Concurrency:** Multi-threaded Rust backend with non-blocking read-only SQLite connections (`sqlite3` bundled statically)
- **Link & Cache Optimization:** `mold` high-performance linker + `sccache` + optimized codegen units

---

## 🚀 Getting Started

### Prerequisites

- **Node.js:** v22+ (uses native `node:fs` globbing)
- **Rust:** Stable toolchain (`rustc`, `cargo`)
- **Linux Packages (Debian/Ubuntu):**
  ```bash
  sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

### Local Development

```bash
# Clone the repository
git clone https://github.com/internetdozero/OrquestraAI.git
cd OrquestraAI

# Install frontend dependencies
npm install

# Run frontend in development mode
npm run dev

# Run full desktop application in development mode
npm run dev:tauri

# Run fast incremental dev build (embedded custom protocol)
npm run build:fast
```

### Running Tests

```bash
# Run unit and contract test suite
npm run test:all

# Run Rust sessions and harness tests directly
cargo test --lib
```

---

## 📄 License

This project is licensed under the [MIT License](LICENSE) — see the [LICENSE](LICENSE) file for details.
