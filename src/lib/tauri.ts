import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { HarnessSummary } from "./harnesses";
import type { SkillItem } from "./skills";

export interface CoreStatus {
  name: string;
  version: string;
  platform: string;
  bridge: string;
}

function detectTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Ask the Rust core for its identity so the footer can prove the IPC bridge is
 * live. In a plain browser preview the invoke is skipped and `isTauri` is false.
 */
export function useCoreStatus() {
  const [status, setStatus] = useState<CoreStatus | null>(null);
  const [isTauri] = useState(detectTauriRuntime);

  useEffect(() => {
    if (!isTauri) return;

    let cancelled = false;

    invoke<CoreStatus>("core_status")
      .then((value) => {
        if (!cancelled) setStatus(value);
      })
      .catch(() => {
        if (!cancelled) setStatus(null);
      });

    return () => {
      cancelled = true;
    };
  }, [isTauri]);

  return { status, isTauri };
}

/** List skills evaluated for `harnessId`; `global` evaluates the baseline. */
export function listSkills(harnessId: string): Promise<SkillItem[]> {
  return invoke<SkillItem[]>("list_skills", {
    harnessId: harnessId === "global" ? null : harnessId,
  });
}

export function setSkillState(
  id: string,
  harnessId: string,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("set_skill_state", { id, harnessId, enabled });
}

export function batchSetAll(
  harnessId: string,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("batch_set_all", { harnessId, enabled });
}

export function harnessSummary(): Promise<HarnessSummary[]> {
  return invoke<HarnessSummary[]>("harness_summary");
}

export function readMasterAgentsMd(): Promise<string> {
  return invoke<string>("read_master_agents_md");
}

export function writeMasterAgentsMd(content: string): Promise<void> {
  return invoke<void>("write_master_agents_md", { content });
}
