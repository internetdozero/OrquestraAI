import { ArrowUpDown, Loader2, Power, PowerOff } from "lucide-react";
import type { McpSortMode } from "../lib/mcps";
import { GLOBAL_SCOPE } from "../lib/harnesses";
import { useI18n } from "../hooks/useI18n";
import { CustomDropdown } from "./CustomDropdown";
import { actionButtonClass } from "./ui";

/** Order control for the MCP grid, sat with the other grid controls. */
function SortSelect({
  value,
  onChange,
}: {
  value: McpSortMode;
  onChange: (mode: McpSortMode) => void;
}) {
  const { t } = useI18n();

  const options: { id: McpSortMode; label: string }[] = [
    { id: "active", label: t.mcps.sortOptions.active },
    { id: "alpha", label: t.mcps.sortOptions.alpha },
    { id: "alphaDesc", label: t.mcps.sortOptions.alphaDesc },
    { id: "official", label: t.mcps.sortOptions.official },
  ];

  return (
    <CustomDropdown
      value={value}
      options={options}
      onChange={onChange}
      ariaLabel={t.mcps.sortMcps}
      title={t.mcps.orderGrid}
      icon={ArrowUpDown}
    />
  );
}

interface McpToolbarProps {
  /** Harness id, or `global`. */
  scope: string;
  scopeLabel: string;
  visibleCount: number;
  totalCount: number;
  sortMode: McpSortMode;
  onSortModeChange: (mode: McpSortMode) => void;
  /** Batch buttons are live (bridge attached, list loaded, nothing pending). */
  canBatch: boolean;
  batchPending: boolean;
  onApplyAll: (enabled: boolean) => void;
}

/**
 * Control cluster above the MCP grid: result count, sort order and the
 * atomic bulk projections for the active scope.
 *
 * Everything here is `flex-wrap`: on compact windows the cluster drops to
 * its own row instead of squeezing the grid or clipping the sort menu.
 */
export function McpToolbar({
  scope,
  scopeLabel,
  visibleCount,
  totalCount,
  sortMode,
  onSortModeChange,
  canBatch,
  batchPending,
  onApplyAll,
}: McpToolbarProps) {
  const { t } = useI18n();
  const isGlobal = scope === GLOBAL_SCOPE.id;

  return (
    <div className="flex min-w-0 flex-wrap items-center justify-end gap-2">
      <SortSelect value={sortMode} onChange={onSortModeChange} />
      <span className="font-mono text-[10.5px] whitespace-nowrap text-zinc-500">
        {t.mcps.shownCount(visibleCount)}
        <span className="text-zinc-600"> / {totalCount}</span>
      </span>
      <button
        type="button"
        onClick={() => onApplyAll(true)}
        disabled={!canBatch}
        className={actionButtonClass}
        title={t.mcps.enableAllTitle(scopeLabel)}
      >
        <Power className="mr-1 inline size-3" />
        {t.mcps.enableAll}
      </button>
      <button
        type="button"
        onClick={() => onApplyAll(false)}
        disabled={!canBatch}
        className={actionButtonClass}
        title={
          isGlobal
            ? t.mcps.disableAllGlobal
            : t.mcps.disableAllHarness(scopeLabel)
        }
      >
        {batchPending ? (
          <Loader2 className="mr-1 inline size-3 animate-spin" />
        ) : (
          <PowerOff className="mr-1 inline size-3" />
        )}
        {t.mcps.disableAll}
      </button>
    </div>
  );
}
