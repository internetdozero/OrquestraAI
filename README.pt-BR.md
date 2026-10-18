# OrquestraAI

<div align="center">

**Painel e Plano de Controle Universal para IA e Harnesses de Código**

[![License: MIT](https://img.shields.io/badge/Licen%C3%A7a-MIT-blue.svg)](LICENSE)
[![Status: Alpha](https://img.shields.io/badge/Status-Alpha%20(Desenvolvimento%20Ativo)-orange.svg)](https://github.com/internetdozero/OrquestraAI)
[![Platform](https://img.shields.io/badge/Plataforma-Linux%20%7C%20Tauri%20v2-emerald.svg)](https://github.com/internetdozero/OrquestraAI)

[**Read in English / Ler em Inglês (README.md)**](README.md)

</div>

---

> [!WARNING]
> **VERSÃO ALPHA — EM DESENVOLVIMENTO ATIVO**
> O OrquestraAI está atualmente em fase **Alpha**. Arquitetura, APIs internas e telas estão evoluindo com frequência. Embora a ponte de harnesses, a biblioteca de skills, a telemetria de sessões e a gestão de MCPs já estejam amplamente testadas e prontas para uso local, migrações de esquema ou alterações estruturais podem acontecer entre versões alpha.

---

## ⚡ Instalação Rápida

```bash
curl -fsSL https://internetdozero.com.br/orquestra/install.sh | bash
```

---

## 🌟 O que é o OrquestraAI?

A rotina de engenharia com agentes de IA modernos costuma envolver múltiplos assistentes e CLIs simultaneamente: **Claude Code**, **OpenAI Codex**, **xAI Grok**, **Antigravity (AGY)**, **Freebuff / Manicode**, **GitHub Copilot CLI**, **Hermes Agent**, **OpenCode** e **Kilo Code**.

No entanto, cada ferramenta isola suas próprias pastas de skills, configurações de servidores MCP, instruções de sistema (`AGENTS.md` vs `CLAUDE.md`) e históricos de conversa.

O **OrquestraAI** resolve essa fragmentação através de uma aplicação desktop em **Tauri v2 + Rust + React** e CLI headless de alto desempenho:
1. **Cofre Universal de Skills e Projeções:** Mantenha um repositório central (`~/.orquestra/skills-library`) e projete skills dinamicamente (via symlinks atômicos ou injeção em configurações JSONC) em qualquer harness sem duplicar arquivos no disco.
2. **Catálogo de Servidores MCP Global e por Harness:** Ative, desative e configure servidores do Model Context Protocol (MCP) globalmente ou por agente individual.
3. **Protocolo Master `AGENTS.md`:** Mantenha suas regras e instruções comportamentais unificadas entre todas as ferramentas sem precisar copiar e colar arquivos manualmente.
4. **Telemetria de Sessões e Leitor de Transcripts Cross-Harness:** Acompanhe espaço em disco, contagem de turnos e feeds de conversa de todas as 9 IAs, com sanitização (limpeza de tags de raciocínio interno e extração limpa dos prompts do usuário).
5. **Janitor (Guarda de Sistema e Memória):** Evita travamentos por falta de memória RAM (OOM Killer), varre processos órfãos de agentes e faz a limpeza segura de caches transitórios.
6. **Idea Vault e Quadro Inter-Agentes:** Bloco de notas e ponte de comunicação assíncrona para fluxos de múltiplos agentes.

---

## 🛠️ Harnesses Suportados (Os 9 Agentes Principais)

| Harness | Caminho de Projeção de Skills | Arquivo de Instruções | Integração de Configuração / Banco de Dados |
| :--- | :--- | :--- | :--- |
| **Antigravity (AGY)** | `~/.gemini/config/skills` | `~/.gemini/config/AGENTS.md` | Logs de sessão (`.system_generated/logs/transcript.jsonl`) |
| **OpenAI Codex** | `~/.codex/skills` | `~/.codex/AGENTS.md` | `thread_history_1.sqlite` + rollouts multi-arquivos |
| **Claude Code** | `~/.claude/skills` | `~/.claude/CLAUDE.md` | `projects/<slug>/*.jsonl` |
| **Freebuff / Manicode** | `~/.freebuff/skills`, `~/.config/manicode/skills` | `~/.config/manicode/AGENTS.md` | `desktop-v2.db` + histórico Manicode + worktrees |
| **OpenCode** | Injeção via `skills.paths` | Instruções nativas | `~/.config/opencode/opencode.jsonc` + `opencode.db` |
| **Kilo Code** | Injeção via `skills.paths` | Instruções nativas | `~/.config/kilo/kilo.jsonc` + `kilo.db` |
| **GitHub Copilot CLI** | `~/.copilot/skills` | Instruções nativas | `session-store.db` (tabela de turns) |
| **Hermes Agent** | `~/.hermes/skills` | Instruções nativas | `state.db` (tabela de messages) |
| **xAI Grok** | `~/.grok/skills` | Instruções nativas | `chat_history.jsonl` + índice SQLite |

---

## 🏗️ Arquitetura e Tecnologias

- **Shell / Núcleo Nativo:** [Tauri v2](https://v2.tauri.app/) (Rust 2021, GTK/WebKit nativo no Linux)
- **Frontend:** React 19 + TypeScript + Vite 8
- **Estilização:** Tailwind CSS v4 (Paleta escura personalizada Zinc/Slate com temas dinâmicos)
- **Concorrência e Estado:** Backend Rust multithread com conexões SQLite somente-leitura não-bloqueantes (`sqlite3` embutido estaticamente)
- **Otimização de Linkagem e Cache:** Linker `mold` de alta performance + `sccache` + unidades de codegen otimizadas

---

## 🚀 Como Executar

### Pré-requisitos

- **Node.js:** v22+ (utiliza recursos nativos de globbing em `node:fs`)
- **Rust:** Toolchain estável (`rustc`, `cargo`)
- **Dependências Linux (Debian/Ubuntu):**
  ```bash
  sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

### Desenvolvimento Local

```bash
# Clone o repositório
git clone https://github.com/internetdozero/OrquestraAI.git
cd OrquestraAI

# Instale as dependências do frontend
npm install

# Inicie o frontend em modo dev
npm run dev

# Inicie o aplicativo desktop completo em modo dev
npm run dev:tauri

# Compilação rápida de desenvolvimento (com protocolo customizado embutido)
npm run build:fast
```

### Executando Testes

```bash
# Executa toda a suite de testes unitários e contratos
npm run test:all

# Executa os testes do core Rust diretamente
cargo test --lib
```

---

## 📄 Licença

Este projeto está sob a [Licença MIT](LICENSE) — consulte o arquivo [LICENSE](LICENSE) para mais detalhes.
