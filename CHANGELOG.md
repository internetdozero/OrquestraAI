# Changelog

All notable changes to **OrquestraAI** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### ⚠️ Aviso de Segurança — Rotação de Chaves Legadas & Expurgo de Backups

- **Backups `.migrate-bak` em texto puro:** a migração para a criptografia em repouso (AES-256-GCM) preservou cópias legíveis dos segredos em `vault.json.migrate-bak` e `mcps.json.migrate-bak` como rede de segurança. Esses arquivos ficam no disco com modo `0600`, mas qualquer segredo salvo antes da `v0.1.0-alpha.5` pode ter transitado em texto puro.
- **Ação recomendada:** (1) confirme que seus serviços carregam normalmente; (2) use o botão **"Purge plaintext backups"** na tela de Credenciais (comando `vault_purge_migration_backups`, com confirmação explícita) para sobrescrever os backups com zeros e excluí-los; (3) **rotacione as chaves de API dos provedores** salvas antes da `alpha.5` nos respectivos painéis (GitHub, OpenAI, Anthropic, Cloudflare etc.). A exclusão é _best-effort_ em SSDs e filesystems copy-on-write — a rotação elimina o risco residual.
- O expurgo nunca destrói a única cópia legível: cada backup só é apagado se o arquivo lacrado correspondente ainda abrir sob a chave atual.

## [0.1.0-alpha.5] - 2026-10-22

> **Resumo da Versão:** Consolidação dos avanços entre `alpha.3` e `alpha.5`. Esta versão eleva o OrquestraAI para uma plataforma segura de execução: introduz o **Blind Vault** com criptografia em repouso AES-256-GCM, motor de streaming de transcripts linha a linha (eliminando picos de memória em históricos massivos), concorrência atômica com autoridade `flock`, updater assinado com Ed25519 nativo e portões de segurança 100% limpos no CodeQL. _(A release `alpha.4` foi imediatamente substituída por esta versão para resolução de alertas estáticos)._

### 🔐 Blind Vault & Segurança de Credenciais

- **Criptografia em Repouso (AES-256-GCM):** Chaves e segredos em `vault.json` e `mcps.json` agora são lacrados via AES-256-GCM com chave derivada do Keyring do SO (Secret Service, Keychain, Credential Manager) ou fallback seguro `0600`.
- **Proxy Cego de Execução (`vault_http_request` / `vault_exec`):** Chamadas HTTP e ferramentas MCP injetam segredos no nível nativo em Rust, impedindo que credenciais transitem pela memória JavaScript ou pelo contexto dos LLMs.
- **Proteção Anti-Exfiltração & Allowlist:** Requisições blindadas validam domínios HTTPS permitidos por preset e aplicam scrubbing em múltiplas codificações (Base64, URL, Hex) contra vazamentos em logs e respostas.
- **Isolamento de Capacidades IPC:** Escopos granulares no Tauri v2 restringem comandos sensíveis da janela principal.

### ⚡ Performance & Streaming

- **Streaming de Sessões Linha a Linha:** Leitura de arquivos `.jsonl` e rollouts (Claude, Codex, Grok) convertida para streaming sob demanda (`for_each_jsonl`), eliminando carregamento de arquivos inteiros na RAM.
- **Virtualização de Transcripts:** Feed com renderização virtualizada e busca com debounce, mantendo fluidez mesmo em conversas gigantescas.
- **Divisão de Pacotes (_Bundle Splitting_):** Lazy-loading de modais e diálogos secundários, acelerando a inicialização da aplicação.
- **Otimização de Projeções:** Resolução em lote (`projected_ids`) substituindo centenas de chamadas de canonicalização por uma única passada em disco.

### 🛡️ Concorrência & Robustez do Sistema

- **Instância Única Confiável (`flock`):** Trava nativa em nível de SO (`.orquestra.lock`) com encerramento de sockets e limpeza de sinais em reinicializações.
- **Gravações Atômicas Concorrentes:** Sufixos temporários únicos por thread e processo (`unique_tmp_suffix`), prevenindo entrelaçamento de bytes em gravações concorrentes de IPC.
- **Updater Nativo Assinado via Ed25519:** Download e verificação de integridade via crates nativas do Rust (sem dependência de `curl`/`wget`), com assinatura criptográfica `SHA256SUMS.txt.minisig`.
- **CSP Estrita:** Remoção de `unsafe-inline` em estilos e adequação a padrões de Content Security Policy seguros.

### 🎨 Experiência do Usuário (UI/UX)

- **Status Dots no Vault:** Indicadores visuais circulares de integridade com rampa de cor independente do tema de destaque.
- **Melhorias no Idea Vault:** Chips `#id` em fonte mono, indicador visual `NEW` para tarefas recentes, confirmação ágil com tecla Enter e persistência via cache SWR.
- **Guardrail de Animações:** Verificação automatizada pré-commit (`scripts/check-card-motion.mjs`) que barra `transition: all` desnecessário na UI.

### 🧩 Ecossistema de Skills & MCPs

- **MCP Oficial de Busca:** Integração e registro do servidor oficial `orquestra-web-search`.
- **Compatibilidade Freebuff:** Adoção de `~/.agents/skills` como raiz canônica sem colisão com outros harnesses da frota.
- **Auditoria de Segurança Ativa:** Inspeção recursiva de scripts maliciosos em tempo real, sem falsos positivos ou bypasses de cache.
- **Deduplicação de Aliases:** Supressão automática de pastas legadas quando o pacote canônico `orquestra-*` está ativo.

### 🐛 Correções & Qualidade

- **CodeQL (Zero Alertas Abertos):** Geração dinâmica de chaves e nonces via `Aes256Gcm` e `OsRng`, encerrando todos os alertas de valores criptográficos estáticos.
- **Prettier & Cargo Fmt:** Sincronização e formatação estrita do contrato IPC e tipagens TypeScript.
- **Tratamento de Erros no Frontend:** Error Boundaries com recuperação graciosa e log estruturado no backend.

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
