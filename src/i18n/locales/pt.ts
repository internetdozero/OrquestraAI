import { ptRest } from "./pt-rest";

/**
 * Master Portuguese dictionary.
 *
 * Its inferred type is the single source of truth for the whole app: `en.ts`
 * is annotated with `TranslationSchema`, so adding a key here fails the build
 * until the English copy exists too.
 */

const pt = {
  common: {
    tagline: "Plano de Controle Universal de Harnesses",
    searchPlaceholder: "Buscar skills, categorias, descrições…",
    searchAriaLabel: "Buscar skills",
    refreshSkills: "Atualizar skills",
    rescanVault: "Reescanear o cofre",
    activeSkills: "Skills ativas",
    agentsChip: "AGENTS.md",
    language: "Idioma",
    languageNames: {
      pt: "Português (Brasil)",
      en: "English",
    },
    ...ptRest.common,
  },
  scopes: ptRest.scopes,

  sidebar: {
    workspace: "Espaço de trabalho",
    counts: (skills: number, harnesses: number) =>
      `${skills} skills · ${harnesses} harnesses`,
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
    },
    inheritance: {
      title: "Herança",
      body: "Skills globais permanecem travadas como ativas em todos os lugares. Desative-as no Global para liberar o controle por harness. Escolha o escopo nas abas dentro de cada visão.",
    },
  },

  skills: {
    activeSkills: "Skills Ativas",
    tokensSaved: "Tokens Salvos / Turno",
    disabledSkills: "Skills Desativadas",
    projectedBaseline: "Projetadas na linha de base universal",
    effectiveInHarness: "Efetivas neste harness",
    tokensSavedHint:
      "Nome e descrição das desativadas saem de todo turno. O arquivo inteiro só entra se a skill for aberta.",
    indexedVault: (total: number) => `${total} skills indexadas no cofre`,
    lockedByGlobalCount: (count: number) => `${count} travadas pelo Global`,
    scopeLabel: "Escopo",
    configuring: (label: string) => `Configurando: ${label}`,
    universalBaselineDesc: (harnesses: number) =>
      `Linha de base universal — skills ativadas são herdadas e travadas em todos os ${harnesses} harnesses.`,
    inheritedDesc: (count: number, label: string) =>
      `${count} skill${count === 1 ? "" : "s"} herdada${count === 1 ? "" : "s"} do Global · controles abaixo aplicam-se apenas a ${label}.`,
    isolatedDesc: (label: string) =>
      `Nenhuma skill herdada no momento. Controles abaixo aplicam-se apenas a ${label}.`,
    sortSkills: "Ordenar skills",
    orderGrid: "Ordenar grade de skills",
    sortOptions: {
      active: "Ativas Primeiro",
      alpha: "Ordem Alfabética (A-Z)",
      tokensDesc: "Mais Tokens Primeiro",
      tokensAsc: "Menos Tokens Primeiro",
    },
    shownCount: (count: number) => `${count} exibidas`,
    enableAll: "Ativar todas",
    disableAll: "Desativar todas",
    symlinkAllInto: (label: string) => `Criar symlinks de todas as skills em ${label}`,
    unlinkAllGlobal: "Remover symlinks de todas as skills do Global e de todos os harnesses",
    unlinkAllHarness: (label: string) => `Remover symlinks de todas as skills não herdadas de ${label}`,
    activeGroup: "Ativas",
    idleGroup: "Inativas",
    inheritedFromGlobal: "Herdada do Global",
    inheritedLockTitle: "Herdada do Global — desative no escopo Global para desbloquear",
    activePill: "Ativa",
    idlePill: "Inativa",
    globalProjection: "projeção global",
    symlinkedHere: "vinculada aqui",
    notProjected: "não projetada",
    harnessNotInstalled: "harness não instalado",
    activeSkillsCount: (active: number, total: number) => `${active} de ${total} skills ativas`,
    symlinkedFrom: "As skills ativas são vinculadas de",
    into: "para",
    lockedNote: "Skills travadas herdam do Global.",
    enableVerb: "ativar",
    disableVerb: "desativar",
    toggleFailed: (verb: string, name: string, detail: string) =>
      `Não foi possível ${verb} “${name}”: ${detail}`,
    batchFailed: (detail: string) => `Falha na atualização em lote: ${detail}`,
    stateUnavailableTitle: "Ponte desktop indisponível",
    stateUnavailableDesc:
      "As skills são escaneadas e vinculadas pelo núcleo em Rust. Execute o app com npm run tauri dev, ou abra o binário compilado, para ler seu cofre.",
    stateReadErrorTitle: "Não foi possível ler o cofre de skills",
    stateEmptyVaultTitle: "Seu cofre está vazio",
    stateEmptyVaultDesc: "Adicione pastas contendo um SKILL.md dentro de",
    stateEmptyVaultSuffix: ", e depois reescaneie.",
    stateNoMatchTitle: (query: string) => `Nenhuma skill corresponde a “${query}”`,
    stateNoMatchDesc: "Tente um id de skill, categoria ou palavra da descrição.",
    tryAgain: "Tentar novamente",
    clearSearch: "Limpar busca",
  },

  mcps: {
    registryTitle: "Registro MCP",
    headerChip: "Registro MCP",
    subtitle:
      "Um catálogo em ~/.orquestra/mcps.json, compilado no dialeto de cada harness. O Global é a linha de base; um harness pode aderir por cima dele.",
    active: "Ativos",
    addServer: "Adicionar servidor MCP",
    recompile: "Recompilar os arquivos de configuração de todos os harnesses",
    reading: "Lendo o catálogo…",
    emptyTitle: "Nenhum servidor MCP ainda",
    emptyBody:
      "Adicione o primeiro servidor e o OrquestraAI o compilará em cada harness instalado que tenha um adaptador.",
    targets: "Destinos",
    ...ptRest.mcps,
  },
  agents: ptRest.agents,
  tokens: ptRest.tokens,
  bridge: ptRest.bridge,

  footer: {
    systemOnline: "Sistema online",
    previewMode: "Modo de visualização",
    core: "Núcleo",
    browserPreview: "Prévia no navegador",
    checking: "Verificando…",
    connectedAll: (count: number) =>
      `Conectado (todos os ${count} harnesses vinculados)`,
    notConnected: "Não conectado",
    linkedOf: (bridged: number, total: number) =>
      `${bridged} de ${total} vinculados`,
    connectBridge: "Conectar ponte de harnesses",
    linking: "Vinculando…",
    harnessBridge: "Ponte de harnesses",
    manageHint: "Revisar o plano de vínculo e o backup dos harnesses",
    previewHint: "A ponte de harnesses é executada no aplicativo desktop",
  },

  presets: {
    title: "Presets de Missão",
    save: "Salvar",
    saveTitle: "Salvar a configuração atual como preset",
    saveHeading: "Salvar configuração atual",
    saveBody:
      "Nomeia as skills e os servidores MCP ativos agora para restaurá-los em um clique.",
    nameLabel: "Nome",
    descriptionLabel: "Descrição",
    optional: "(opcional)",
    saveAction: "Salvar preset",
    cancel: "Cancelar",
    saved: (name: string) => `Preset “${name}” salvo`,
    applyFailed: (name: string, detail: string) =>
      `Não foi possível aplicar “${name}”: ${detail}`,
    deleteTitle: (name: string) => `Excluir ${name}`,
    deleted: (name: string) => `Preset “${name}” excluído`,
    ...ptRest.presets,
  },
};

export type TranslationSchema = typeof pt;

export default pt;
