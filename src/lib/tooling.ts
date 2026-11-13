import { invoke } from "@tauri-apps/api/core";
import { loadCacheValue, reloadCacheValue } from "./dataCache";

/** One external program the core shells out to. */
export interface ToolProbe {
  id: string;
  present: boolean;
  /** First line of `<tool> --version`; null when the tool is missing. */
  version: string | null;
  /** Feature ids that stop working without it. */
  disables: string[];
  /** Install command for this platform, when the tool is unavailable. */
  hint: string | null;
}

export interface ToolReport {
  tools: ToolProbe[];
  missing: string[];
  /** True inside a Flatpak sandbox, where host binaries are invisible. */
  sandboxed: boolean;
  /** Environment note (for example the Flatpak limitation), when relevant. */
  note: string | null;
}

/**
 * Tools change installation state, not minute to minute: probing on every
 * Settings visit walked the PATH again for an answer that only moves when the
 * operator installs something.
 */
export const TOOL_PROBE_KEY = "tools:probe";
export const TOOL_PROBE_TTL_MS = 5 * 60_000;

/** Probe every external tool so a missing one explains itself before it is used. */
export function probeExternalTools(options?: { force?: boolean }): Promise<ToolReport> {
  const request = {
    key: TOOL_PROBE_KEY,
    ttlMs: TOOL_PROBE_TTL_MS,
    load: () => invoke<ToolReport>("probe_external_tools"),
  };
  return options?.force ? reloadCacheValue(request) : loadCacheValue(request);
}
