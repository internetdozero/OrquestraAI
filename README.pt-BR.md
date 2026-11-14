# OwnHub

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Status: Alpha](https://img.shields.io/badge/Status-Alpha-orange.svg)](CHANGELOG.md)
[![Platforms](https://img.shields.io/badge/Platforms-Linux%20%7C%20macOS%20%7C%20Windows-emerald.svg)](https://github.com/internetdozero/OwnHub/releases/latest)

[Read in English](README.md)

O OwnHub é um app desktop e uma CLI para gerenciar vários agentes de código com IA num lugar só. Ele mantém skills, servidores MCP, arquivos de instruções, sessões e credenciais consistentes entre Claude Code, OpenAI Codex, xAI Grok, Antigravity (AGY), GitHub Copilot CLI, Hermes Agent, OpenCode e Kilo Code.

> [!WARNING]
> **Software em Alpha.** Formatos de configuração, comandos IPC e a saída da CLI podem mudar entre releases. Fixe automações numa release com tag.

## Recursos

- **Cofre de skills:** uma biblioteca em `~/.ownhub/skills-library`, projetada em cada harness por symlinks ou entradas nativas de configuração.
- **Registro de MCPs:** ative servidores MCP globalmente ou por harness. Definições já existentes do Gemini/Antigravity são adotadas, e os servidores oficiais são instalados num ambiente privado em `~/.ownhub/mcp-servers/`.
- **Instruções unificadas:** um `AGENTS.md` mestre ligado ao caminho de instruções de cada harness.
- **Sessões:** navegue, busque, exporte, fixe e limpe históricos de conversa de todos os harnesses suportados.
- **Cofre de credenciais:** chaves de API cifradas em repouso com AES-256-GCM. A chave de cifragem fica no keyring do sistema. Agentes podem usar as chaves por um proxy cego de HTTP/exec sem vê-las, e chaves copiadas na interface saem do clipboard após 20 segundos.
- **Idea Vault e blackboard:** lista de tarefas e notas compartilhadas para coordenar agentes.
- **Janitor:** monitora memória e arquivos temporários e limpa processos de agentes órfãos.
- **Runtime portátil:** ferramentas externas são resolvidas pelo `PATH` do login shell além do ambiente do processo, então instalações via `nvm`, `mise`, Homebrew e `~/.local/bin` funcionam quando o app é aberto pelo menu. O que faltar é reportado com o comando exato de instalação.

## Instalação

Binários publicados: **Linux x86_64**, **Linux aarch64**, **macOS Apple Silicon**, **macOS Intel** e **Windows x86_64** (instalador NSIS e o executável avulso que o atualizador substitui).

```bash
curl -fsSL https://ownhub.internetdozero.com.br/install.sh | bash
```

O instalador:

1. Resolve a release mais recente do GitHub e baixa o binário, o `SHA256SUMS.txt` e o `SHA256SUMS.txt.minisig`.
2. Verifica a assinatura minisign com a chave de release embutida **antes** de confiar em qualquer checksum. Isso exige o [`minisign`](https://jedisct1.github.io/minisign/) ou Python 3 com PyNaCl (`python3-nacl` no Debian/Ubuntu). A instalação para se nenhum dos dois estiver disponível ou se a assinatura for inválida.
3. Confere o SHA-256 do binário e instala em `~/.local/bin/ownhub`.
4. No Linux, verifica as bibliotecas WebKitGTK/libsoup e também as ferramentas de runtime (`xdg-utils` ou `gio`, `git`, `python3` com `venv`), instalando o que faltar pelo gerenciador de pacotes usando `sudo`. Use `--no-auto-deps` para pular a instalação e cuidar disso você mesmo.

O suporte de distribuição depende da glibc: a release é compilada no Ubuntu 22.04, então Debian 11, Ubuntu 20.04 e RHEL/Rocky 8 param no loader. Um bundle Flatpak feito do mesmo binário roda nessas distribuições. Veja [docs/install.md](docs/install.md) para a tabela de versões e o canal Flatpak.

Para diagnosticar uma instalação Linux existente sem baixar de novo:

```bash
curl -fsSL https://ownhub.internetdozero.com.br/install.sh | sh -s -- --check-deps
```

Binários e notas de versão ficam na [página de Releases](https://github.com/internetdozero/OwnHub/releases). As mudanças estão no [CHANGELOG](CHANGELOG.md).

## CLI

O mesmo binário roda subcomandos sem interface. Adicione `--json` para saída legível por máquina.

```bash
ownhub sessions list --harness claude
ownhub skills list --scope claude
ownhub mcp status
ownhub vault list
ownhub idea list --project MeuProjeto
```

| Comando              | Cobre                                                                                  | Guia                                         |
| :------------------- | :------------------------------------------------------------------------------------- | :------------------------------------------- |
| `ownhub sessions` | listar, inspecionar, buscar, exportar, fixar, limpar, política de retenção, acompanhar | [docs/cli/sessions.md](docs/cli/sessions.md) |
| `ownhub skills`   | listar, info, ativar/desativar, criar, importar (local ou git), exportar, validar      | [docs/cli/skills.md](docs/cli/skills.md)     |
| `ownhub mcp`      | listar, status, adicionar/remover, ativar/desativar, deploy, listar e chamar tools     | [docs/cli/mcp.md](docs/cli/mcp.md)           |
| `ownhub vault`    | credenciais, pools de chaves, rotação, testes de provedor, proxy cego, auditoria       | [docs/cli/vault.md](docs/cli/vault.md)       |
| `ownhub idea`     | ciclo de ideias, busca, métricas, notas do blackboard                                  | [docs/cli/ideas.md](docs/cli/ideas.md)       |

`ownhub --help` lista os atalhos de bridge e MCP (`--bridge-status`, `--bridge-deploy`, `--mcp-status`, `--mcp-set`, `--mcp-deploy`, `--preset`).

## Harnesses suportados

| Harness            | Skills                    | Instruções                   | Sessões / config                      |
| :----------------- | :------------------------ | :--------------------------- | :------------------------------------ |
| Antigravity (AGY)  | `~/.gemini/config/skills` | `~/.gemini/config/AGENTS.md` | logs de transcript, `mcp_config.json` |
| OpenAI Codex       | `~/.codex/skills`         | `~/.codex/AGENTS.md`         | `thread_history_1.sqlite`, rollouts   |
| Claude Code        | `~/.claude/skills`        | `~/.claude/CLAUDE.md`        | `projects/<slug>/*.jsonl`             |
| OpenCode           | `skills.paths` na config  | nativo                       | `opencode.jsonc`, `opencode.db`       |
| Kilo Code          | `skills.paths` na config  | nativo                       | `kilo.jsonc`, `kilo.db`               |
| GitHub Copilot CLI | `~/.copilot/skills`       | nativo                       | `session-store.db`                    |
| Hermes Agent       | `~/.hermes/skills`        | nativo                       | `state.db`                            |
| xAI Grok           | `~/.grok/skills`          | nativo                       | `chat_history.jsonl`                  |

Só arquivos de nível de usuário são gerenciados. Pastas `.agents` de workspace não são importadas.

## Desenvolvimento

Requisitos: Node.js 22+, Rust estável e, no Debian/Ubuntu:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

```bash
git clone https://github.com/internetdozero/OwnHub.git
cd OwnHub
npm install
npm run dev:tauri        # app desktop em modo de desenvolvimento
```

Verificações que o CI roda:

```bash
npm test                 # testes unitários e de contrato em Node
npm run lint && npm run format:check
cargo fmt --all -- --check
cargo test --workspace
```

Stack: Tauri v2 e um workspace Rust (`crates/`: core, harness, vault, bridge, sessions) com frontend React 19 + TypeScript + Vite estilizado com Tailwind CSS v4. Veja [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/IPC_API.md](docs/IPC_API.md), [docs/vault.md](docs/vault.md) e [docs/security.md](docs/security.md).

As skills e servidores MCP oficiais ficam em [internetdozero/ownhub-extensions](https://github.com/internetdozero/ownhub-extensions).

## Licença

[MIT](LICENSE)
