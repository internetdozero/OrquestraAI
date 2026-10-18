/** Harness bridge: link plan, rationales and deploy reports. */
export const bridge = {
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
};
