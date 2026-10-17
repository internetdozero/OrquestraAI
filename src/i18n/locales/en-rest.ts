import { ptRest } from "./pt-rest";

/** English mirror of `pt-rest.ts`. A missing or extra key fails the build. */
export const enRest: typeof ptRest = {
  common: {
    close: "Close",
    dismiss: "Dismiss",
    unexpected: "Unexpected error",
    delete: "Delete",
    edit: "Edit",
    notInstalled: "not installed",
    loadingResources: "Loading resources...",
    loadingResourcesHint:
      "Essentials come in now. Everything else keeps loading in the background.",
  },

  scopes: {
    globalHint: "Universal baseline inherited by every harness",
  },

  mcps: {
    unavailableBody:
      "The MCP registry is compiled by the Rust core into each harness config. Run the desktop shell to read or change it.",
    readFailed: (detail: string) => `Could not read the MCP catalog: ${detail}`,
    toggled: (name: string, verb: string, scope: string) =>
      `${name} ${verb} · ${scope}`,
    enabledVerb: "enabled",
    disabledVerb: "disabled",
    deletedFromConfigs: (name: string) =>
      `${name} deleted from every harness config`,
    deleteFailed: (name: string, detail: string) =>
      `Could not delete ${name}: ${detail}`,
    savedDeployed: (name: string) => `${name} saved and deployed`,
    alreadyMatches: "Every harness config already matches the catalog",
    recompiled: (count: number) =>
      `Recompiled ${count} config${count === 1 ? "" : "s"}`,
    recompileFailed: (detail: string) => `Recompile failed: ${detail}`,
    noAdapters: "no adapters resolved",
    notInstalledMark: "(not installed)",
    inactive: "Inactive",
    onElsewhere: "on elsewhere",
    noDescription: "No description",
    inspect: "Command, args & env",
    deleteConfirm: (name: string) =>
      `Delete “${name}” from every harness config?`,
    deleteAria: (name: string) => `Delete ${name}`,
    inheritedShort: "inherited",
    inScope: (label: string) => `in ${label}`,
    toggleInherited: (label: string) => `${label} is inherited from Global`,
    toggleEnable: (label: string) => `Enable ${label}`,
    toggleDisable: (label: string) => `Disable ${label}`,
    revealValue: "Reveal value",
    closeDetails: "Close execution details",
    detailsAria: (name: string) => `${name} execution details`,
    executionDetails: "Execution details",
    scopeOf: (label: string) => `scope ${label}`,
    command: "Command",
    environment: "Environment",
    arguments: "Arguments",
    noneDeclared: "None declared",
    enabledIn: "Enabled in",
    globalBadge: "global",
    nowhereYet: "nowhere yet",
    editServer: "Edit server",
    editTitle: (name: string) => `Edit ${name}`,
    formBlurb: "One definition, compiled into every harness format it is enabled for.",
    fieldName: "Name",
    fieldId: "Id",
    fieldDescription: "Description",
    argsHint: "one per line",
    envHint: "KEY=VALUE per line",
    envNote:
      "Values are written to the harness config verbatim; files that end up holding them are tightened to 0600.",
    enableGlobally: "Enable globally",
    enableGloballyHint: "every adapted harness inherits it",
    nameRequired: "A display name is required.",
    idRequired: "An id is required.",
    idCharset:
      "Ids must be lowercase letters, digits, dots, dashes or underscores.",
    idTaken: (id: string) => `“${id}” already exists in the catalog.`,
    commandRequired: "A launch command is required.",
    addServerAction: "Add server",
    saveChanges: "Save changes",
    descriptionPlaceholder: "What this server exposes to the model",
  },

  agents: {
    configuring: "Configuring",
    masterHint: (count: number) =>
      `Master instructions — one source of truth for ${count} linked harnesses.`,
    unsaved: "Unsaved changes",
    saved: "Saved",
    saveDeploy: "Save & Deploy",
    lineCount: (count: number) => `${count} lines`,
    charCount: (count: number) => `${count} chars`,
    editorAria: "AGENTS.md editor",
    loadingPlaceholder: "Loading master instructions…",
    emptyPlaceholder: "Write the instructions every linked harness should follow…",
    reading: "Reading master instructions…",
    linkedHarnesses: "Linked harnesses",
    masterFile: "master file",
    unavailableBody:
      "The master AGENTS.md is served by the Rust core. Run the app with npm run tauri dev to edit and save it.",
    loadFailed: (detail: string) => `Could not load AGENTS.md: ${detail}`,
    saveFailed: (detail: string) => `Could not save AGENTS.md: ${detail}`,
    savedToast: "AGENTS.md saved to ~/.orquestra/AGENTS.md",
  },

  bridge: {
    projected: "projected",
    quarantined: "quarantined",
    archived: "archived",
    leaking: "leaking",
    notInstalled: "Not installed",
    manual: "Manual",
    linked: "Linked",
    leakingSkills: "Leaking skills",
    unlinked: "Unlinked",
    untouched: "Harness is not installed — the bridge leaves it untouched.",
    title: "Connect the Harness Bridge",
    summary: (managed: number, linked: number, pending: number) => {
      const found = `${managed} installed harness${managed === 1 ? "" : "es"} detected`;
      const done = `${linked} already linked`;
      const waiting = pending > 0 ? ` · ${pending} waiting` : "";
      return `${found} · ${done}${waiting}`;
    },
    warnLead:
      "Every skill folder, instruction file, and config file listed below is moved into",
    warnTail: "before a symlink is created. Nothing is deleted.",
    warnSees:
      "After linking, a harness only sees the skills you enabled for it in this app. Skills that exist only inside a harness are adopted into the vault first.",
    warnSecondary:
      "Secondary discovery locations — vendor checkouts, plugin marketplaces, and bundled skill repositories — are archived into the vault, then emptied or replaced with the projection link so no skill loads behind your back.",
    linkPlan: "Link plan",
    masterLabel: "master AGENTS.md · ~/.orquestra/AGENTS.md",
    reversible: "Reversible from the backup folder",
    backupConnect: "Back up & Connect",
    linkingHarnesses: "Linking harnesses…",
    rationales: {
      sharedRoot: "Shared user root that Codex and Grok inject into every session",
      vendorBuiltins:
        "Vendor built-ins ship with the app; archived in the vault so ~/.gemini/config/skills stays the only user source",
      codexVendor: "Curated vendor checkout Codex indexes beside ~/.codex/skills",
      bundledMarket: "Bundled marketplace cache of plugin skills",
      staleMarket: "Stale marketplace cache of plugin skills",
      claudePlugins: "Plugin marketplace skills load beside ~/.claude/skills",
      hermesOptional:
        "Optional skill repository Hermes auto-loads beside ~/.hermes/skills",
      grokBundled: "Grok resolves its built-ins here and ignores ~/.grok/skills",
      grokPlugins: "Third-party plugin bundles Grok loads on startup",
    },
    notes: {
      codexSystem: "Internal .system folders are moved into the projection first",
      freebuff: "Manicode shares the Freebuff projection",
      hermesCurator: "Curator state files are kept inside the projection",
    },
    readFailed: (detail: string) => `Could not read the harness bridge: ${detail}`,
    partial: (errors: string) => `Harness bridge partially deployed: ${errors}`,
    backupDetail: (name: string) => `backup ${name}`,
    adopted: (count: number) =>
      `${count} harness-only skill${count === 1 ? "" : "s"} adopted`,
    imported: (sources: string) => `AGENTS.md imported from ${sources}`,
    connected: (bridged: number, total: number, extra: string) =>
      `Harness bridge connected — ${bridged}/${total} harnesses bridged${extra}`,
    deployFailed: (detail: string) =>
      `Could not deploy the harness bridge: ${detail}`,
  },

  presets: {
    everySkill: "every skill",
    skillCount: (count: number) => `${count} skills`,
    everyMcp: "every MCP",
    mcpCount: (count: number) => `${count} MCPs`,
    applied: (name: string, skills: number, mcps: number) =>
      `${name} applied · ${skills} skills · ${mcps} MCPs`,
    readFailed: (detail: string) => `Could not read the presets: ${detail}`,
    saveFailed: (detail: string) => `Could not save the preset: ${detail}`,
    deleteFailed: (name: string, detail: string) =>
      `Could not delete “${name}”: ${detail}`,
    namePlaceholder: "Game Dev",
    descriptionPlaceholder: "Engine work, no cloud tooling",
    warnSkillMissing: (id: string) => `skill “${id}” is not in the vault`,
    warnMcpMissing: (id: string) => `MCP server “${id}” is not in the catalog`,
    warnDeploy: (detail: string) => `MCP deploy incomplete: ${detail}`,
    warnBridge: (detail: string) => `harness bridge not refreshed: ${detail}`,
    builtins: {
      stealth: {
        name: "Stealth",
        description:
          "Zero-bloat profile: one lightweight skill plus the core MCPs, so a rate-limited model keeps its whole context window.",
      },
      "rust-dev": {
        name: "Rust Dev",
        description:
          "Systems and low-level work: repository, module and knowledge servers without the frontend or design tooling.",
      },
      "web-fullstack": {
        name: "Web Fullstack",
        description:
          "Frontend and cloud work: UI generation, browser automation, design assets and the deployment edge.",
      },
      "full-power": {
        name: "Full Power",
        description:
          "Every skill in the vault and every MCP server in the catalog: nothing held back.",
      },
    },
  },

  tokens: {
    entry: "input",
    full: "full",
    unit: "tok",
    entryHint: "Name and description sent on every turn while the item is on.",
    fullSkillHint:
      "Whole SKILL.md, estimated. It enters context only when the skill is opened.",
    fullMcpHint:
      "Whole catalog definition: name, description, command and arguments. Env secrets and the live server schema are not included.",
    agentsHint: "Estimate of the whole file. AGENTS.md is sent on every turn.",
    scopeHint: "Estimated sum of the items active in this scope.",
  },
};
