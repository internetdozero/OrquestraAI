import type { TranslationSchema } from "../pt";

/** English mirror of `pt/mcps.ts`. A missing or extra key fails the build. */
export const mcps: TranslationSchema["mcps"] = {
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
  exploreStoreAction: "Explore the official store",
  addManualAction: "Add manually",
  targets: "Targets",
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
  headers: "Headers",
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
  smartPasteTitle: "Smart paste",
  smartPasteHint:
    "Paste a terminal command (e.g. npx -y …) or a Claude JSON block, or tap a preset.",
  smartPastePlaceholder:
    "npx -y @modelcontextprotocol/server-github  or  { \"command\": … }",
  smartPasteAction: "Fill fields",
  smartPasteApplied: "Fields filled — review and save.",
  smartPasteFailed:
    "Could not recognize that text. Paste a CLI command or JSON with \"command\".",
};
