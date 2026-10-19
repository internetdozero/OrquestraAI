import { useCallback } from "react";
import { useI18n } from "./useI18n";
import type { SkillItem } from "../lib/skills";
import {
  importSkillFromGit,
  setSkillState,
  type OfficialMcpEntry,
  type OfficialSkillEntry,
} from "../lib/tauri";
import type {
  StoreRowAction,
  StoreStatus,
} from "../components/OfficialStore";

interface SkillStoreDeps {
  /** Currently selected scope (`global` or harness id) for instant projection. */
  scope: string;
  toggleSkill: (id: string) => Promise<void>;
  invalidate: () => Promise<void>;
  onSummaryStale: () => void;
  onToast?: (message: string) => void;
}

/** Live status + one-click install-and-project actions for the skills hub. */
export function useSkillStoreActions(
  skills: SkillItem[],
  deps: SkillStoreDeps,
) {
  const { t } = useI18n();
  const { scope, toggleSkill, invalidate, onSummaryStale, onToast } = deps;

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
          // One-click install + instant projection: clone into the vault,
          // then atomically symlink into the current scope so the skill is
          // live instead of dormant. Summary counters refresh via the same
          // path as a manual toggle.
          run: async () => {
            const imported = await importSkillFromGit(url);
            try {
              await setSkillState(imported.id, scope, true);
            } catch {
              // Inherited-from-global or missing scope: stay installed idle.
            }
            onSummaryStale();
            await invalidate();
            onToast?.(t.skills.import.success(imported.name));
          },
        };
      }
      return null;
    },
    [t, scope, toggleSkill, invalidate, onSummaryStale, onToast],
  );

  return { statusOf, actionFor };
}
