import { Plus, Waypoints, Wrench } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import { actionButtonClass, StatePanel } from "./ui";

/**
 * Day-1 empty state for the MCP registry: keeps the explanatory copy and
 * offers two in-place resolutions instead of making the user hunt for the
 * store tab or the header button.
 */
export function McpEmptyState({
  onExploreStore,
  onAddManual,
}: {
  onExploreStore: () => void;
  onAddManual: () => void;
}) {
  const { t } = useI18n();

  return (
    <StatePanel icon={Wrench} title={t.mcps.emptyTitle} description={t.mcps.emptyBody}>
      <div className="mt-1 flex flex-wrap items-center justify-center gap-2">
        <button type="button" onClick={onExploreStore} className={actionButtonClass}>
          <Waypoints className="mr-1 inline size-3" />
          {t.mcps.exploreStoreAction}
        </button>
        <button type="button" onClick={onAddManual} className={actionButtonClass}>
          <Plus className="mr-1 inline size-3" />
          {t.mcps.addManualAction}
        </button>
      </div>
    </StatePanel>
  );
}
