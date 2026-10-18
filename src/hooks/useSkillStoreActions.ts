import { useCallback } from "react";
import { useI18n } from "./useI18n";
import type { SkillItem } from "../lib/skills";
import {
  importSkillFromGit,
  type OfficialMcpEntry,
  type OfficialSkillEntry,
} from "../lib/tauri";
import type {
  StoreRowAction,
  StoreStatus,
} from "../components/OfficialStore";

interface SkillStoreDeps {
  toggleSkill: (id: string) => Promise<void>;
  invalidate: () => Promise<void>;
  onSummaryStale: () => void;
  onToast?: (message: string) => void;
}

/** Live status + one-click actions backing the skills store tab. */
export function useSkillStoreActions(
  skills: SkillItem[],
  deps: SkillStoreDeps,
) {
  const { t } = useI18n();
  const { toggleSkill, invalidate, onSummaryStale, onToast } = deps;

  const statusOf = useCallback(
    (id: string): StoreStatus => {
      const found = skills.find((skill) => skill.id === id);
      if (!found) return "missing";
      return found.harnessEnabled ? "active" : "idle";
    },
    [skills],
  );

  const actionFor = useCallback(
    (
      entry: OfficialSkillEntry | OfficialMcpEntry,
      status: StoreStatus,
    ): StoreRowAction | null => {
      if (status === "idle") {
        return { kind: "enable", run: () => toggleSkill(entry.id) };
      }
      if (status === "missing" && entry.repoUrl) {
        const url = entry.repoUrl;
        return {
          kind: "install",
          run: async () => {
            const imported = await importSkillFromGit(url);
            onSummaryStale();
            await invalidate();
            onToast?.(t.skills.import.success(imported.name));
          },
        };
      }
      return null;
    },
    [t, toggleSkill, invalidate, onSummaryStale, onToast],
  );

  return { statusOf, actionFor };
}
