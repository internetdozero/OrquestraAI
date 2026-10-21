import { AlertTriangle, FolderOpen, RefreshCw, SearchX, Unplug } from "lucide-react";
import { vaultPath } from "../lib/skills";
import { useI18n } from "../hooks/useI18n";
import { actionButtonClass, StatePanel } from "./ui";

export type SkillsStateKind = "unavailable" | "read-error" | "empty-vault" | "no-match";

/**
 * Full-panel state for the skill grid. One component keeps the four messages
 * (no bridge, unreadable vault, empty vault, empty search) in the same voice
 * and out of the view's render tree.
 */
export function SkillsState({
  kind,
  error,
  query,
  onRescan,
  onClearQuery,
}: {
  kind: SkillsStateKind;
  error: string | null;
  query: string;
  onRescan: () => void;
  onClearQuery: () => void;
}) {
  const { t } = useI18n();

  switch (kind) {
    case "unavailable":
      return (
        <StatePanel
          icon={Unplug}
          title={t.skills.stateUnavailableTitle}
          description={t.skills.stateUnavailableDesc}
        >
          <code className="rounded border border-white/10 bg-white/[0.03] px-2 py-1 font-mono text-[10.5px] text-zinc-400">
            npm run tauri dev
          </code>
        </StatePanel>
      );

    case "read-error":
      return (
        <StatePanel
          icon={AlertTriangle}
          title={t.skills.stateReadErrorTitle}
          description={error ?? t.common.unexpected}
        >
          <button type="button" onClick={onRescan} className={actionButtonClass}>
            <RefreshCw className="mr-1 inline size-3" />
            {t.skills.tryAgain}
          </button>
        </StatePanel>
      );

    case "empty-vault":
      return (
        <StatePanel
          icon={FolderOpen}
          title={t.skills.stateEmptyVaultTitle}
          description={
            <>
              {t.skills.stateEmptyVaultDesc}{" "}
              <code className="font-mono text-zinc-400">{vaultPath}</code>
              {t.skills.stateEmptyVaultSuffix}
            </>
          }
        >
          <button type="button" onClick={onRescan} className={actionButtonClass}>
            <RefreshCw className="mr-1 inline size-3" />
            {t.common.rescanVault}
          </button>
        </StatePanel>
      );

    case "no-match":
      return (
        <StatePanel
          icon={SearchX}
          title={t.skills.stateNoMatchTitle(query.trim())}
          description={t.skills.stateNoMatchDesc}
        >
          <button type="button" onClick={onClearQuery} className={actionButtonClass}>
            {t.skills.clearSearch}
          </button>
        </StatePanel>
      );
  }
}
