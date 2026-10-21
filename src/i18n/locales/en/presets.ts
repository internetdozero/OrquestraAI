import type { TranslationSchema } from "../pt";

/** English mirror of `pt/presets.ts`. A missing or extra key fails the build. */
export const presets: TranslationSchema["presets"] = {
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
  applyFailed: (name: string, detail: string) => `Could not apply “${name}”: ${detail}`,
  deleteTitle: (name: string) => `Delete ${name}`,
  deleted: (name: string) => `Preset “${name}” deleted`,
  editHeading: "Edit Preset",
  editBody: "Modify properties, skills, and MCP servers for this preset.",
  editAction: "Save changes",
  editTitle: (name: string) => `Edit ${name}`,
  resetToDefault: "Reset to default",
  resetConfirm: (name: string) => `Reset preset “${name}” to factory defaults?`,
  resetSuccess: (name: string) => `Preset “${name}” reset to factory defaults`,
  resetFailed: (name: string, detail: string) => `Could not reset “${name}”: ${detail}`,
  updateSuccess: (name: string) => `Preset “${name}” updated`,
  updateFailed: (name: string, detail: string) => `Could not update “${name}”: ${detail}`,
  iconLabel: "Icon",
  filterItems: "Filter skills or MCPs…",
  allSkillsSwitch: "All skills",
  allMcpsSwitch: "All MCPs",
  builtinBadge: "Built-in",
  harnessScopesKept: "Per-harness scopes kept",
  everySkill: "every skill",
  skillCount: (count: number) => `${count} skills`,
  everyMcp: "every MCP",
  mcpCount: (count: number) => `${count} MCPs`,
  applied: (name: string, skills: number, mcps: number) =>
    `${name} applied · ${skills} skills · ${mcps} MCPs`,
  readFailed: (detail: string) => `Could not read the presets: ${detail}`,
  saveFailed: (detail: string) => `Could not save the preset: ${detail}`,
  deleteFailed: (name: string, detail: string) => `Could not delete “${name}”: ${detail}`,
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
};
