import type { TranslationSchema } from "../pt";

/** English mirror of `pt/common.ts`. A missing or extra key fails the build. */
export const common: TranslationSchema["common"] = {
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
  close: "Close",
  titlebar: {
    minimize: "Minimize",
    maximize: "Maximize",
    restore: "Restore",
    closeWindow: "Close window",
  },
  dismiss: "Dismiss",
  unexpected: "Unexpected error",
  viewCrashedHint:
    "This view hit an unexpected error. Your data is safe — retry the view or reload the app.",
  reloadApp: "Reload app",
  delete: "Delete",
  edit: "Edit",
  notInstalled: "not installed",
  loadingResources: "Loading resources...",
  loadingResourcesHint: "Essentials come in now. Everything else keeps loading in the background.",
};

export const scopes: TranslationSchema["scopes"] = {
  globalHint: "Universal baseline inherited by every harness",
};

export const sidebar: TranslationSchema["sidebar"] = {
  workspace: "Workspace",
  counts: (skills: number, harnesses: number) => `${skills} skills · ${harnesses} harnesses`,
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
    sessions: {
      label: "Sessions",
      hint: "Unified chat history across every harness",
    },
    ideas: {
      label: "Ideas",
      hint: "Idea vault and inter-agent blackboard",
    },
    credentials: {
      label: "Credentials",
      hint: "Blind API key vault",
    },
  },
  credentials: {
    label: "Credentials",
    hint: "Blind API key vault with failover",
  },
  inheritance: {
    title: "Inheritance",
    body: "Global skills stay locked ON everywhere. Turn them off in Global to unlock per-harness control. Pick the scope with the tabs inside each view.",
  },
};

export const footer: TranslationSchema["footer"] = {
  systemOnline: "System Online",
  previewMode: "Preview Mode",
  core: "Core",
  browserPreview: "Browser preview",
  checking: "Checking…",
  connectedAll: (count: number) => `Connected (All ${count} Harnesses Bridged)`,
  notConnected: "Not connected",
  linkedOf: (bridged: number, total: number) => `${bridged} of ${total} linked`,
  connectBridge: "Connect Harness Bridge",
  linking: "Linking…",
  harnessBridge: "Harness Bridge",
  manageHint: "Review the harness link plan and backup",
  previewHint: "The harness bridge runs in the desktop app",
};

export const agents: TranslationSchema["agents"] = {
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
};

export const store: TranslationSchema["store"] = {
  official: "Official",
  libraryTab: "Library",
  storeTab: "Official Store",
  title: "Official Store",
  subtitle: "Skills and MCP servers maintained by OrquestraAI, with one-click install.",
  install: "Install",
  enable: "Enable",
  add: "Add",
  installed: "Installed",
  unpublished: "No published repository yet",
  loading: "Loading the official store…",
  loadFailed: (detail: string) => `Could not load the official store: ${detail}`,
  viewRepo: "View repository",
  actionFailed: (detail: string) => `The store action failed: ${detail}`,
  filterAll: "All",
  filterOfficial: "Official",
  filterCommunity: "Community",
  verifiedOfficial: "Verified Official",
  communityWithScripts: "Community • Contains executable scripts",
  communityPromptOnly: "Community • Prompt only",
  installAndProject: "Install & Project",
  projected: "Projected",
  projectHint: (scope: string) => `Projects into ${scope} right after install`,
  scriptsWarning: (count: number) =>
    `${count} executable script${count === 1 ? "" : "s"} — review before installing`,
  communitySubtitle: "Community curation with script audit and one-click projection.",
};

export const tokens: TranslationSchema["tokens"] = {
  entry: "input",
  full: "full",
  unit: "tok",
  entryHint: "Name and description sent on every turn while the item is on.",
  fullSkillHint: "Whole SKILL.md, estimated. It enters context only when the skill is opened.",
  fullMcpHint:
    "Whole catalog definition: name, description, command and arguments. Env secrets and the live server schema are not included.",
  agentsHint: "Estimate of the whole file. AGENTS.md is sent on every turn.",
  scopeHint: "Estimated sum of the items active in this scope.",
};
