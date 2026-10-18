import { Plus, Waypoints, Wrench } from "lucide-react";
import { useI18n } from "../hooks/useI18n";

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
    <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed border-white/10 bg-zinc-900/20 px-6 py-16 text-center">
      <div className="grid size-10 place-items-center rounded-full border border-white/[0.07] bg-white/[0.03]">
        <Wrench className="size-4 text-zinc-500" />
      </div>
      <p className="text-sm font-semibold text-zinc-200">{t.mcps.emptyTitle}</p>
      <p className="max-w-sm text-[12px] leading-relaxed text-zinc-500">
        {t.mcps.emptyBody}
      </p>
      <div className="mt-1 flex flex-wrap items-center justify-center gap-2">
        <button
          type="button"
          onClick={onExploreStore}
          className="flex items-center gap-1.5 rounded-lg border border-emerald-500/30 bg-emerald-500/15 px-3 py-2 text-[11.5px] font-medium text-emerald-200 shadow-[0_0_24px_-8px_rgba(16,185,129,0.6)] transition hover:bg-emerald-500/25"
        >
          <Waypoints className="size-3.5" />
          {t.mcps.exploreStoreAction}
        </button>
        <button
          type="button"
          onClick={onAddManual}
          className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.02] px-3 py-2 text-[11.5px] font-medium text-zinc-300 transition hover:border-white/20 hover:text-zinc-100"
        >
          <Plus className="size-3.5" />
          {t.mcps.addManualAction}
        </button>
      </div>
    </div>
  );
}
