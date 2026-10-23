/**
 * Shared chrome: global actions, scope tabs, sidebar, footer, master
 * instructions and token units. `index.ts` re-exposes the sibling sections
 * as the top-level `scopes`, `sidebar`, `footer`, `agents` and `tokens`
 * paths, so every component accessor stays exactly as it is today.
 */
export const common = {
  tagline: "Plano de Controle Universal de Harnesses",
  searchPlaceholder: "Buscar skills, categorias, descrições…",
  searchAriaLabel: "Buscar skills",
  searchPlaceholderMcps: "Buscar servidores MCP, comandos, descrições…",
  searchAriaLabelMcps: "Buscar servidores MCP",
  refreshSkills: "Atualizar skills",
  rescanVault: "Reescanear o cofre",
  activeSkills: "Skills ativas",
  agentsChip: "AGENTS.md",
  language: "Idioma",
  languageNames: {
    pt: "Português (Brasil)",
    en: "English",
  },
  close: "Fechar",
  titlebar: {
    minimize: "Minimizar",
    maximize: "Maximizar",
    restore: "Restaurar",
    closeWindow: "Fechar janela",
  },
  dismiss: "Dispensar",
  unexpected: "Erro inesperado",
  viewCrashedHint:
    "Esta visão encontrou um erro inesperado. Seus dados estão a salvo — tente de novo ou recarregue o app.",
  reloadApp: "Recarregar app",
  delete: "Excluir",
  edit: "Editar",
  notInstalled: "não instalado",
  loadingResources: "Carregando recursos...",
  loadingResourcesHint: "O essencial entra agora. O restante carrega em segundo plano.",
};

export const scopes = {
  globalHint: "Linha de base universal herdada por todos os harnesses",
};

export const sidebar = {
  workspace: "Espaço de trabalho",
  counts: (skills: number, harnesses: number) => `${skills} skills · ${harnesses} harnesses`,
  views: {
    skills: {
      label: "Skills",
      hint: "Projeções de skills, por escopo de harness",
    },
    mcps: {
      label: "Servidores MCP",
      hint: "Servidores MCP compilados em cada harness",
    },
    agents: {
      label: "AGENTS.md",
      hint: "Regras mestras projetadas em cada harness",
    },
    sessions: {
      label: "Sessões",
      hint: "Histórico unificado de conversas em cada harness",
    },
    ideas: {
      label: "Ideias",
      hint: "Baú de ideias e quadro entre agentes",
    },
    credentials: {
      label: "Credenciais",
      hint: "Cofre cego de chaves de API",
    },
  },
  credentials: {
    label: "Credenciais",
    hint: "Cofre cego de chaves de API com failover",
  },
  inheritance: {
    title: "Herança",
    body: "Skills globais permanecem travadas como ativas em todos os lugares. Desative-as no Global para liberar o controle por harness. Escolha o escopo nas abas dentro de cada visão.",
  },
};

export const footer = {
  systemOnline: "Sistema online",
  previewMode: "Modo de visualização",
  core: "Núcleo",
  browserPreview: "Prévia no navegador",
  checking: "Verificando…",
  connectedAll: (count: number) => `Conectado (todos os ${count} harnesses vinculados)`,
  notConnected: "Não conectado",
  linkedOf: (bridged: number, total: number) => `${bridged} de ${total} vinculados`,
  connectBridge: "Conectar ponte de harnesses",
  linking: "Vinculando…",
  harnessBridge: "Ponte de harnesses",
  manageHint: "Revisar o plano de vínculo e o backup dos harnesses",
  previewHint: "A ponte de harnesses é executada no aplicativo desktop",
};

export const agents = {
  configuring: "Configurando",
  masterHint: (count: number) =>
    `Instruções mestras — uma fonte da verdade para ${count} harnesses vinculados.`,
  unsaved: "Alterações não salvas",
  saved: "Salvo",
  saveDeploy: "Salvar e implantar",
  lineCount: (count: number) => `${count} linhas`,
  charCount: (count: number) => `${count} caracteres`,
  editorAria: "Editor do AGENTS.md",
  loadingPlaceholder: "Carregando instruções mestras…",
  emptyPlaceholder: "Escreva as instruções que cada harness vinculado deve seguir…",
  reading: "Lendo instruções mestras…",
  linkedHarnesses: "Harnesses vinculados",
  masterFile: "arquivo mestre",
  unavailableBody:
    "O AGENTS.md mestre é servido pelo núcleo em Rust. Execute o app com npm run tauri dev para editar e salvar.",
  loadFailed: (detail: string) => `Não foi possível carregar o AGENTS.md: ${detail}`,
  saveFailed: (detail: string) => `Não foi possível salvar o AGENTS.md: ${detail}`,
  savedToast: "AGENTS.md salvo em ~/.orquestra/AGENTS.md",
};

export const store = {
  official: "Oficial",
  libraryTab: "Biblioteca",
  storeTab: "Loja oficial",
  title: "Loja oficial",
  subtitle: "Skills e servidores MCP mantidos pelo OrquestraAI, com instalação em um clique.",
  install: "Instalar",
  enable: "Ativar",
  add: "Adicionar",
  installed: "Instalado",
  unpublished: "Sem repositório publicado ainda",
  loading: "Carregando loja oficial…",
  loadFailed: (detail: string) => `Não foi possível carregar a loja oficial: ${detail}`,
  viewRepo: "Ver repositório",
  actionFailed: (detail: string) => `A ação da loja falhou: ${detail}`,
  filterAll: "Todas",
  filterOfficial: "Oficiais",
  filterCommunity: "Comunidade",
  verifiedOfficial: "Oficial verificada",
  communityWithScripts: "Comunidade • Contém scripts executáveis",
  communityPromptOnly: "Comunidade • Somente prompt",
  installAndProject: "Instalar e projetar",
  projected: "Projetada",
  projectHint: (scope: string) => `Projeta em ${scope} logo após instalar`,
  scriptsWarning: (count: number) =>
    `${count} script${count === 1 ? "" : "s"} executável${count === 1 ? "" : "es"} — revise antes de instalar`,
  communitySubtitle: "Curadoria da comunidade com auditoria de scripts e projeção em um clique.",
};

export const tokens = {
  entry: "entrada",
  full: "inteiro",
  unit: "tok",
  entryHint: "Nome e descrição que entram em todo turno enquanto o item está ativo.",
  fullSkillHint: "SKILL.md inteiro, estimado. Só entra no contexto quando a skill é aberta.",
  fullMcpHint:
    "Definição inteira do catálogo: nome, descrição, comando e argumentos. Não inclui segredos de ambiente nem o schema ao vivo do servidor.",
  agentsHint: "Estimativa do arquivo inteiro. O AGENTS.md entra em todo turno.",
  scopeHint: "Soma estimada dos itens ativos neste escopo.",
};
