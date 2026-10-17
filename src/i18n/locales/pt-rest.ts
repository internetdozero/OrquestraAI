/**
 * Portuguese copy for the surfaces the first dictionary pass left hardcoded.
 * Composed into `pt.ts`; `en-rest.ts` must mirror every key.
 */
export const ptRest = {
  common: {
    close: "Fechar",
    dismiss: "Dispensar",
    unexpected: "Erro inesperado",
    delete: "Excluir",
    edit: "Editar",
    notInstalled: "não instalado",
    loadingResources: "Carregando recursos...",
    loadingResourcesHint:
      "O essencial entra agora. O restante carrega em segundo plano.",
  },

  scopes: {
    globalHint: "Linha de base universal herdada por todos os harnesses",
  },

  mcps: {
    unavailableBody:
      "O registro MCP é compilado pelo núcleo em Rust na configuração de cada harness. Abra o aplicativo desktop para ler ou alterar.",
    readFailed: (detail: string) =>
      `Não foi possível ler o catálogo MCP: ${detail}`,
    toggled: (name: string, verb: string, scope: string) =>
      `${name} ${verb} · ${scope}`,
    enabledVerb: "ativado",
    disabledVerb: "desativado",
    deletedFromConfigs: (name: string) =>
      `${name} excluído de todos os arquivos de configuração`,
    deleteFailed: (name: string, detail: string) =>
      `Não foi possível excluir ${name}: ${detail}`,
    savedDeployed: (name: string) => `${name} salvo e implantado`,
    alreadyMatches: "Todos os arquivos de configuração já batem com o catálogo",
    recompiled: (count: number) =>
      count === 1
        ? "Recompilado 1 arquivo de configuração"
        : `Recompilados ${count} arquivos de configuração`,
    recompileFailed: (detail: string) => `Falha ao recompilar: ${detail}`,
    noAdapters: "nenhum adaptador resolvido",
    notInstalledMark: "(não instalado)",
    inactive: "Inativo",
    onElsewhere: "ativo em outro lugar",
    noDescription: "Sem descrição",
    inspect: "Comando, argumentos e ambiente",
    deleteConfirm: (name: string) =>
      `Excluir “${name}” de todos os arquivos de configuração?`,
    deleteAria: (name: string) => `Excluir ${name}`,
    inheritedShort: "herdada",
    inScope: (label: string) => `em ${label}`,
    toggleInherited: (label: string) => `${label} é herdada do Global`,
    toggleEnable: (label: string) => `Ativar ${label}`,
    toggleDisable: (label: string) => `Desativar ${label}`,
    revealValue: "Revelar valor",
    closeDetails: "Fechar detalhes de execução",
    detailsAria: (name: string) => `Detalhes de execução de ${name}`,
    executionDetails: "Detalhes de execução",
    scopeOf: (label: string) => `escopo ${label}`,
    command: "Comando",
    environment: "Ambiente",
    arguments: "Argumentos",
    noneDeclared: "Nenhum declarado",
    enabledIn: "Ativo em",
    globalBadge: "global",
    nowhereYet: "em lugar nenhum ainda",
    editServer: "Editar servidor",
    editTitle: (name: string) => `Editar ${name}`,
    formBlurb:
      "Uma definição, compilada em cada formato de harness em que estiver ativa.",
    fieldName: "Nome",
    fieldId: "Id",
    fieldDescription: "Descrição",
    argsHint: "um por linha",
    envHint: "KEY=VALUE por linha",
    envNote:
      "Os valores são gravados na configuração do harness como estão; arquivos que os guardam ficam com permissão 0600.",
    enableGlobally: "Ativar globalmente",
    enableGloballyHint: "todo harness com adaptador herda",
    nameRequired: "Um nome de exibição é obrigatório.",
    idRequired: "Um id é obrigatório.",
    idCharset:
      "O id deve ter letras minúsculas, dígitos, pontos, traços ou sublinhados.",
    idTaken: (id: string) => `“${id}” já existe no catálogo.`,
    commandRequired: "Um comando de inicialização é obrigatório.",
    addServerAction: "Adicionar servidor",
    saveChanges: "Salvar alterações",
    descriptionPlaceholder: "O que este servidor expõe ao modelo",
  },

  agents: {
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
    emptyPlaceholder:
      "Escreva as instruções que cada harness vinculado deve seguir…",
    reading: "Lendo instruções mestras…",
    linkedHarnesses: "Harnesses vinculados",
    masterFile: "arquivo mestre",
    unavailableBody:
      "O AGENTS.md mestre é servido pelo núcleo em Rust. Execute o app com npm run tauri dev para editar e salvar.",
    loadFailed: (detail: string) =>
      `Não foi possível carregar o AGENTS.md: ${detail}`,
    saveFailed: (detail: string) =>
      `Não foi possível salvar o AGENTS.md: ${detail}`,
    savedToast: "AGENTS.md salvo em ~/.orquestra/AGENTS.md",
  },

  bridge: {
    projected: "projetado",
    quarantined: "em quarentena",
    archived: "arquivado",
    leaking: "vazando",
    notInstalled: "Não instalado",
    manual: "Manual",
    linked: "Vinculado",
    leakingSkills: "Vazando skills",
    unlinked: "Desvinculado",
    untouched: "Harness não instalado — a ponte não o altera.",
    title: "Conectar a ponte de harnesses",
    summary: (managed: number, linked: number, pending: number) => {
      const found =
        managed === 1
          ? "1 harness instalado detectado"
          : `${managed} harnesses instalados detectados`;
      const done = linked === 1 ? "1 já vinculado" : `${linked} já vinculados`;
      const waiting = pending > 0 ? ` · ${pending} aguardando` : "";
      return `${found} · ${done}${waiting}`;
    },
    warnLead:
      "Cada pasta de skill, arquivo de instruções e arquivo de configuração listado abaixo é movido para",
    warnTail: "antes de criar o symlink. Nada é apagado.",
    warnSees:
      "Depois do vínculo, um harness só vê as skills que você ativou para ele neste app. Skills que existem só dentro de um harness entram primeiro no cofre.",
    warnSecondary:
      "Locais secundários de descoberta — checkouts de fornecedor, marketplaces de plugins e repositórios de skills empacotados — são arquivados no cofre e depois esvaziados ou trocados pelo link da projeção, para nenhuma skill carregar por trás.",
    linkPlan: "Plano de vínculo",
    masterLabel: "AGENTS.md mestre · ~/.orquestra/AGENTS.md",
    reversible: "Reversível pela pasta de backup",
    backupConnect: "Fazer backup e conectar",
    linkingHarnesses: "Vinculando harnesses…",
    rationales: {
      sharedRoot:
        "Raiz compartilhada do usuário que Codex e Grok injetam em toda sessão",
      vendorBuiltins:
        "Built-ins do fornecedor vêm com o app; arquivados no cofre para ~/.gemini/config/skills continuar sendo a única fonte do usuário",
      codexVendor:
        "Checkout de fornecedor que o Codex indexa ao lado de ~/.codex/skills",
      bundledMarket: "Cache de marketplace empacotado de skills de plugin",
      staleMarket: "Cache antigo de marketplace de skills de plugin",
      claudePlugins:
        "Skills de marketplace de plugins carregam ao lado de ~/.claude/skills",
      hermesOptional:
        "Repositório opcional de skills que o Hermes carrega sozinho ao lado de ~/.hermes/skills",
      grokBundled:
        "O Grok resolve os built-ins aqui e ignora ~/.grok/skills",
      grokPlugins:
        "Pacotes de plugins de terceiros que o Grok carrega na inicialização",
    },
    notes: {
      codexSystem:
        "Pastas internas .system são movidas para a projeção primeiro",
      freebuff: "O Manicode compartilha a projeção do Freebuff",
      hermesCurator: "Arquivos de estado do curator ficam dentro da projeção",
    },
    readFailed: (detail: string) =>
      `Não foi possível ler a ponte de harnesses: ${detail}`,
    partial: (errors: string) =>
      `Ponte de harnesses implantada em parte: ${errors}`,
    backupDetail: (name: string) => `backup ${name}`,
    adopted: (count: number) =>
      count === 1
        ? "1 skill exclusiva de harness adotada"
        : `${count} skills exclusivas de harness adotadas`,
    imported: (sources: string) => `AGENTS.md importado de ${sources}`,
    connected: (bridged: number, total: number, extra: string) =>
      `Ponte de harnesses conectada — ${bridged}/${total} harnesses vinculados${extra}`,
    deployFailed: (detail: string) =>
      `Não foi possível implantar a ponte de harnesses: ${detail}`,
  },

  presets: {
    everySkill: "todas as skills",
    skillCount: (count: number) => (count === 1 ? "1 skill" : `${count} skills`),
    everyMcp: "todos os MCPs",
    mcpCount: (count: number) => (count === 1 ? "1 MCP" : `${count} MCPs`),
    applied: (name: string, skills: number, mcps: number) =>
      `${name} aplicado · ${skills} skills · ${mcps} MCPs`,
    readFailed: (detail: string) =>
      `Não foi possível ler os presets: ${detail}`,
    saveFailed: (detail: string) =>
      `Não foi possível salvar o preset: ${detail}`,
    deleteFailed: (name: string, detail: string) =>
      `Não foi possível excluir “${name}”: ${detail}`,
    namePlaceholder: "Dev de jogos",
    descriptionPlaceholder: "Trabalho de engine, sem ferramentas de nuvem",
    warnSkillMissing: (id: string) => `a skill “${id}” não está no cofre`,
    warnMcpMissing: (id: string) => `o servidor MCP “${id}” não está no catálogo`,
    warnDeploy: (detail: string) => `implantação MCP incompleta: ${detail}`,
    warnBridge: (detail: string) => `ponte de harnesses não atualizada: ${detail}`,
    builtins: {
      stealth: {
        name: "Furtivo",
        description:
          "Perfil sem excesso: uma skill leve mais os MCPs essenciais, para um modelo com limite de taxa manter a janela de contexto.",
      },
      "rust-dev": {
        name: "Dev Rust",
        description:
          "Trabalho de sistemas e baixo nível: servidores de repositório, módulo e conhecimento, sem as ferramentas de frontend ou design.",
      },
      "web-fullstack": {
        name: "Web fullstack",
        description:
          "Trabalho de frontend e nuvem: geração de UI, automação de navegador, assets de design e a borda de deploy.",
      },
      "full-power": {
        name: "Potência total",
        description:
          "Todas as skills do cofre e todos os servidores MCP do catálogo: nada fica de fora.",
      },
    },
  },

  tokens: {
    entry: "entrada",
    full: "inteiro",
    unit: "tok",
    entryHint: "Nome e descrição que entram em todo turno enquanto o item está ativo.",
    fullSkillHint:
      "SKILL.md inteiro, estimado. Só entra no contexto quando a skill é aberta.",
    fullMcpHint:
      "Definição inteira do catálogo: nome, descrição, comando e argumentos. Não inclui segredos de ambiente nem o schema ao vivo do servidor.",
    agentsHint: "Estimativa do arquivo inteiro. O AGENTS.md entra em todo turno.",
    scopeHint: "Soma estimada dos itens ativos neste escopo.",
  },
};
