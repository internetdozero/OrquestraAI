import { invoke } from "@tauri-apps/api/core";
import type { LucideIcon } from "lucide-react";
import { Cpu, Globe, ShieldAlert, Sparkles, Zap } from "lucide-react";

/** Per-harness specialization: `harness_id -> ids` enabled only in that harness. */
export type HarnessScopeMap = Record<string, string[]>;

/** Mirrors the Rust `Preset` struct. */
export interface Preset {
  id: string;
  name: string;
  description: string;
  /** Icon key the core assigns: `shield`, `cpu`, `globe`, `zap` or `custom`. */
  icon: string;
  skills: string[];
  mcps: string[];
  /** Per-harness skill specialization; absent on presets saved before scoping. */
  harnessSkills?: HarnessScopeMap;
  /** Per-harness MCP specialization; absent on presets saved before scoping. */
  harnessMcps?: HarnessScopeMap;
  allSkills: boolean;
  allMcps: boolean;
  /** Shipped profiles cannot be deleted. */
  builtin: boolean;
  /** The profile most recently applied. */
  active: boolean;
}

/** Mirrors the Rust `PresetReport` struct returned by `apply_preset`. */
export interface PresetReport {
  id: string;
  name: string;
  skillsEnabled: number;
  skillsDisabled: number;
  mcpsEnabled: number;
  mcpsDisabled: number;
  /** Harness config files the MCP deploy re-compiled. */
  configsChanged: string[];
  /** Non-fatal gaps: unknown ids, a partial deploy, a stale bridge. */
  warnings: string[];
}

const PRESET_ICONS: Record<string, LucideIcon> = {
  shield: ShieldAlert,
  cpu: Cpu,
  globe: Globe,
  zap: Zap,
  custom: Sparkles,
};

/** Glyph for a preset's icon key, with a sensible fallback for custom ones. */
export function presetIcon(icon: string): LucideIcon {
  return PRESET_ICONS[icon] ?? Sparkles;
}

export function listPresets(): Promise<Preset[]> {
  return invoke<Preset[]>("list_presets");
}

/** Apply `id`; the core reconciles projections and recompiles harness configs. */
export function applyPreset(id: string): Promise<PresetReport> {
  return invoke<PresetReport>("apply_preset", { id });
}

/** Snapshot the current live combination as a named custom preset. */
export function savePreset(
  name: string,
  description?: string | null,
): Promise<Preset> {
  return invoke<Preset>("save_preset", { name, description: description ?? null });
}

export function deletePreset(id: string): Promise<void> {
  return invoke<void>("delete_preset", { id });
}

/** Update an existing preset (custom or built-in override). */
export function updatePreset(preset: Preset): Promise<Preset> {
  return invoke<Preset>("update_preset", { preset });
}

/** Reset a built-in preset to original factory defaults. */
export function resetPreset(id: string): Promise<Preset> {
  return invoke<Preset>("reset_preset", { id });
}
