import type { TranslationSchema } from "./pt";
import { enRest } from "./en-rest";

/**
 * English dictionary. Annotating it with `TranslationSchema` makes a missing
 * key, an extra key or a mismatched parameter list a compile error.
 */
const en: TranslationSchema = {
  common: {
    tagline: "Universal Harness Control Plane",
    searchPlaceholder: "Search skills, categories, descriptions…",
    searchAriaLabel: "Search skills",
    refreshSkills: "Refresh skills",
    rescanVault: "Rescan the vault",
    activeSkills: "Active Skills",
    agentsChip: "AGENTS.md",
    language: "Language",
    languageNames: {
      pt: "Português (Brasil)",
      en: "English",
    },
    ...enRest.common,
  },
  scopes: enRest.scopes,

  sidebar: {
    workspace: "Workspace",
    counts: (skills: number, harnesses: number) =>
      `${skills} skills · ${harnesses} harnesses`,
    views: {
      skills: {
        label: "Skills",
        hint: "Skill projections, per harness scope",
      },
      mcps: {
        label: "MCP Servers",
        hint: "MCP servers compiled into every harness",
      },
      agents: {
        label: "AGENTS.md",
        hint: "Master rules projected into every harness",
      },
    },
    inheritance: {
      title: "Inheritance",
      body: "Global skills stay locked ON everywhere. Turn them off in Global to unlock per-harness control. Pick the scope with the tabs inside each view.",
    },
  },

  skills: {
    activeSkills: "Active Skills",
    tokensSaved: "Tokens Saved / Turn",
    disabledSkills: "Disabled Skills",
    projectedBaseline: "Projected into the universal baseline",
    effectiveInHarness: "Effective in this harness",
    tokensSavedHint:
      "Disabled skills drop their name and description from every turn. The full file loads only when a skill is opened.",
    indexedVault: (total: number) => `${total} skills indexed in the vault`,
    lockedByGlobalCount: (count: number) => `${count} locked by Global`,
    scopeLabel: "Scope",
    configuring: (label: string) => `Configuring: ${label}`,
    universalBaselineDesc: (harnesses: number) =>
      `Universal baseline — enabled skills are inherited and locked across all ${harnesses} harnesses.`,
    inheritedDesc: (count: number, label: string) =>
      `${count} skill${count === 1 ? "" : "s"} inherited from Global · toggles below apply only to ${label}.`,
    isolatedDesc: (label: string) =>
      `No skills inherited right now. Toggles below apply only to ${label}.`,
    sortSkills: "Sort skills",
    orderGrid: "Order the skill grid",
    sortOptions: {
      active: "Active First",
      alpha: "Alphabetical (A-Z)",
      tokensDesc: "Most Tokens First",
      tokensAsc: "Least Tokens First",
    },
    shownCount: (count: number) => `${count} shown`,
    enableAll: "Enable all",
    disableAll: "Disable all",
    symlinkAllInto: (label: string) => `Symlink every skill into ${label}`,
    unlinkAllGlobal: "Unlink every skill from Global and every harness",
    unlinkAllHarness: (label: string) => `Unlink every non-inherited skill from ${label}`,
    activeGroup: "Active",
    idleGroup: "Idle",
    inheritedFromGlobal: "Inherited from Global",
    inheritedLockTitle: "Inherited from Global — turn it off in the Global scope to unlock",
    activePill: "Active",
    idlePill: "Idle",
    globalProjection: "global projection",
    symlinkedHere: "symlinked here",
    notProjected: "not projected",
    harnessNotInstalled: "harness not installed",
    activeSkillsCount: (active: number, total: number) => `${active} of ${total} skills active`,
    symlinkedFrom: "Active skills are symlinked from",
    into: "into",
    lockedNote: "Locked skills inherit from Global.",
    enableVerb: "enable",
    disableVerb: "disable",
    toggleFailed: (verb: string, name: string, detail: string) =>
      `Could not ${verb} “${name}”: ${detail}`,
    batchFailed: (detail: string) => `Batch update failed: ${detail}`,
    stateUnavailableTitle: "Desktop bridge unavailable",
    stateUnavailableDesc:
      "Skills are scanned and symlinked by the Rust core. Run the app with npm run tauri dev, or launch the built binary, to read your vault.",
    stateReadErrorTitle: "Could not read the skills vault",
    stateEmptyVaultTitle: "Your vault is empty",
    stateEmptyVaultDesc: "Add folders containing a SKILL.md into",
    stateEmptyVaultSuffix: ", then rescan.",
    stateNoMatchTitle: (query: string) => `No skills match “${query}”`,
    stateNoMatchDesc: "Try a skill id, category, or a word from its description.",
    tryAgain: "Try again",
    clearSearch: "Clear search",
  },

  mcps: {
    registryTitle: "MCP Registry",
    headerChip: "MCP Registry",
    subtitle:
      "One catalog in ~/.orquestra/mcps.json compiled into each harness's own dialect. Global is the baseline; a harness can opt in on top of it.",
    active: "Active",
    addServer: "Add MCP Server",
    recompile: "Recompile every harness config from the catalog",
    reading: "Reading the catalog…",
    emptyTitle: "No MCP servers yet",
    emptyBody:
      "Add the first server and OrquestraAI will compile it into every installed harness that has an adapter.",
    targets: "Targets",
    ...enRest.mcps,
  },
  agents: enRest.agents,
  tokens: enRest.tokens,
  bridge: enRest.bridge,

  footer: {
    systemOnline: "System Online",
    previewMode: "Preview Mode",
    core: "Core",
    browserPreview: "Browser preview",
    checking: "Checking…",
    connectedAll: (count: number) =>
      `Connected (All ${count} Harnesses Bridged)`,
    notConnected: "Not connected",
    linkedOf: (bridged: number, total: number) =>
      `${bridged} of ${total} linked`,
    connectBridge: "Connect Harness Bridge",
    linking: "Linking…",
    harnessBridge: "Harness Bridge",
    manageHint: "Review the harness link plan and backup",
    previewHint: "The harness bridge runs in the desktop app",
  },

  presets: {
    title: "Mission Presets",
    save: "Save",
    saveTitle: "Save the current skills and MCPs as a preset",
    saveHeading: "Save current configuration",
    saveBody:
      "Names the skills and MCP servers active right now so they can be restored in one click.",
    nameLabel: "Name",
    descriptionLabel: "Description",
    optional: "(optional)",
    saveAction: "Save preset",
    cancel: "Cancel",
    saved: (name: string) => `Preset “${name}” saved`,
    applyFailed: (name: string, detail: string) =>
      `Could not apply “${name}”: ${detail}`,
    deleteTitle: (name: string) => `Delete ${name}`,
    deleted: (name: string) => `Preset “${name}” deleted`,
    ...enRest.presets,
  },
};

export default en;
