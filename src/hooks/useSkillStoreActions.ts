import { useCallback } from "react";
import { useI18n } from "./useI18n";
import type { SkillItem } from "../lib/skills";
import {
  importSkillFromGit,
  setSkillState,
  type OfficialMcpEntry,
  type OfficialSkillEntry,
} from "../lib/tauri";
import type { StoreEntry, StoreRowAction, StoreStatus } from "../components/OfficialStore";

interface SkillStoreDeps {
  /** Currently selected scope (`global` or harness id) for instant projection. */
  scope: string;
  toggleSkill: (id: string) => Promise<void>;
  invalidate: () => Promise<void>;
  onSummaryStale: () => void;
  onToast?: (message: string) => void;
}

/** Live status + one-click install-and-project actions for the skills hub. */
export function useSkillStoreActions(skills: SkillItem[], deps: SkillStoreDeps) {
  const { t } = useI18n();
  const { scope, toggleSkill, invalidate, onSummaryStale, onToast } = deps;

  const statusOf = useCallback(
    (entry: StoreEntry): StoreStatus => {
      // Legacy vault folders satisfy the unified entry: no ghost install
      // button for skills imported under their pre-monorepo id.
      const legacy = "legacyId" in entry ? entry.legacyId : undefined;
      const found = skills.find(
        (skill) => skill.id === entry.id || (legacy && skill.id === legacy),
      );
      if (!found) return "missing";
      return found.harnessEnabled ? "active" : "idle";
    },
    [skills],
  );

  const actionFor = useCallback(
    (entry: OfficialSkillEntry | OfficialMcpEntry, status: StoreStatus): StoreRowAction | null => {
      if (status === "idle") {
        return { kind: "enable", run: () => toggleSkill(entry.id) };
      }
      if (status === "missing" && entry.repoUrl) {
        const urls = [
          entry.repoUrl,
          ...("fallbackRepoUrl" in entry && entry.fallbackRepoUrl ? [entry.fallbackRepoUrl] : []),
        ];
        return {
          kind: "install",
          // One-click install + instant projection: clone into the vault
          // (monorepo anchor first, individual repo fallback), then
          // atomically symlink into the current scope so the skill is
          // live instead of dormant. Summary counters refresh via the same
          // path as a manual toggle.
          run: async () => {
            // Monorepo anchor first; individual repo when it is unpublished.
            let imported;
            try {
              imported = await importSkillFromGit(urls[0]);
            } catch (primary) {
              if (!urls[1]) throw primary;
              imported = await importSkillFromGit(urls[1]);
            }
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
