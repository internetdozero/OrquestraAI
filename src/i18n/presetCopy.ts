import type { Preset } from "../lib/presets";
import type { TranslationSchema } from "./locales/pt";

const BUILTIN_IDS = ["stealth", "rust-dev", "web-fullstack", "full-power"] as const;

type BuiltinId = (typeof BUILTIN_IDS)[number];

function isBuiltinId(id: string): id is BuiltinId {
  return (BUILTIN_IDS as readonly string[]).includes(id);
}

/** Built-in profiles keep stable ids in Rust; the bar shows the active locale. */
export function presentPreset(
  t: TranslationSchema,
  preset: Pick<Preset, "id" | "name" | "description" | "builtin">,
): { name: string; description: string } {
  if (preset.builtin && isBuiltinId(preset.id)) {
    return t.presets.builtins[preset.id];
  }
  return { name: preset.name, description: preset.description };
}

/** Tooltip line for how much a preset turns on. */
export function summarizePreset(
  t: TranslationSchema,
  preset: Pick<Preset, "allSkills" | "skills" | "allMcps" | "mcps">,
): string {
  const skills = preset.allSkills
    ? t.presets.everySkill
    : t.presets.skillCount(preset.skills.length);
  const mcps = preset.allMcps
    ? t.presets.everyMcp
    : t.presets.mcpCount(preset.mcps.length);
  return `${skills} · ${mcps}`;
}

/** Rust warnings stay English; the toast shows the active locale. */
export function localizePresetWarning(t: TranslationSchema, warning: string): string {
  const skill = /^skill “(.+)” is not in the vault$/.exec(warning);
  if (skill) return t.presets.warnSkillMissing(skill[1]);

  const mcp = /^MCP server “(.+)” is not in the catalog$/.exec(warning);
  if (mcp) return t.presets.warnMcpMissing(mcp[1]);

  const deploy = /^MCP deploy incomplete: (.*)$/.exec(warning);
  if (deploy) return t.presets.warnDeploy(deploy[1]);

  const bridge = /^harness bridge not refreshed: (.*)$/.exec(warning);
  if (bridge) return t.presets.warnBridge(bridge[1]);

  return warning;
}
