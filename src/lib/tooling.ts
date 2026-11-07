import { invoke } from "@tauri-apps/api/core";

/** One external program the core shells out to. */
export interface ToolProbe {
  id: string;
  present: boolean;
  /** First line of `<tool> --version`; null when the tool is missing. */
  version: string | null;
  /** Feature ids that stop working without it. */
  disables: string[];
}

export interface ToolReport {
  tools: ToolProbe[];
  missing: string[];
}

/** Probe `git` and `tar` so a missing tool explains itself before it is used. */
export function probeExternalTools(): Promise<ToolReport> {
  return invoke<ToolReport>("probe_external_tools");
}
