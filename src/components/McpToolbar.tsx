import { ArrowUpDown, CircleSlash, Loader2, Plug, Plus, Power, PowerOff, Zap } from "lucide-react";
import type { McpSortMode } from "../lib/mcps";
import { GLOBAL_SCOPE } from "../lib/harnesses";
import { formatTokens } from "../lib/utils";
import { useI18n } from "../hooks/useI18n";
import { CustomDropdown } from "./CustomDropdown";
import { StoreTabs } from "./StoreTabs";
import { actionButtonClass, ToolbarSearchInput, ViewModeToggle } from "./ui";

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
  activeCount: number;
  totalCount: number;
  inactiveCount: number;
  entryTokens: number;
  fullTokens: number;
  sortMode: McpSortMode;
  onSortModeChange: (mode: McpSortMode) => void;
  /** Batch buttons are live (bridge attached, list loaded, nothing pending). */
  canBatch: boolean;
  batchPending: boolean;
  onApplyAll: (enabled: boolean) => void;
  onAdd: () => void;
  /** Library | Official Store switch, sat with the other grid controls. */
  showStore: boolean;
  onShowStoreChange: (show: boolean) => void;
  /** Live search text owned by the parent view. */
  query: string;
  onQueryChange: (query: string) => void;
  /** Grid | list switch for the server grid. */
  view: "grid" | "list";
  onViewChange: (view: "grid" | "list") => void;
}

/**
 * Dashboard head for one scope: the metric strip and a single control row
 * (Library | Store switch, sort, bulk projections), mirroring SkillsToolbar.
 *
 * Everything here is `flex-wrap` with `min-w-0` children: at the 900px minimum
 * window the control cluster drops to its own row instead of squeezing the
 * metric strip or clipping the sort menu.
 */
export function McpToolbar({
  scope,
  scopeLabel,
  activeCount,
  totalCount,
  inactiveCount,
  entryTokens,
  fullTokens,
  sortMode,
  onSortModeChange,
  canBatch,
  batchPending,
  onApplyAll,
  onAdd,
  showStore,
  onShowStoreChange,
  query,
  onQueryChange,
  view,
  onViewChange,
}: McpToolbarProps) {
  const { t } = useI18n();
  const isGlobal = scope === GLOBAL_SCOPE.id;

  return (
    <>
      <section
        aria-label={`${t.mcps.active}, ${t.mcps.scopeTokens}, ${t.mcps.inactive}`}
        className="mb-4 flex min-w-0 flex-col gap-y-2 rounded-xl border border-white/[0.06] bg-zinc-900/30 px-4 py-3 @min-[680px]:flex-row @min-[680px]:flex-wrap @min-[680px]:items-center @min-[680px]:gap-x-8"
      >
        <p className="flex w-full items-center gap-2 text-[11px] font-medium tracking-wider uppercase @min-[680px]:w-auto @min-[680px]:shrink-0">
          <Plug className="size-3.5 shrink-0 text-zinc-500" />
          <span className="text-zinc-500">{t.mcps.active} </span>
          <span className="ml-auto font-mono text-[13px] text-zinc-100 @min-[680px]:ml-0">
            {activeCount} / {totalCount}
          </span>
        </p>
        <p className="flex w-full items-center gap-2 text-[11px] font-medium tracking-wider uppercase @min-[680px]:w-auto @min-[680px]:shrink-0">
          <Zap className="size-3.5 shrink-0 text-zinc-500" />
          <span className="text-zinc-500">{t.mcps.scopeTokens} </span>
          <span
            className="ml-auto font-mono text-[13px] text-zinc-100 normal-case @min-[680px]:ml-0"
            title={t.tokens.scopeHint}
          >
            {t.tokens.entry} ≈ {formatTokens(entryTokens)} · {t.tokens.full} ≈{" "}
            {formatTokens(fullTokens)} {t.tokens.unit}
          </span>
        </p>
        <p className="flex w-full items-center gap-2 text-[11px] font-medium tracking-wider uppercase @min-[680px]:w-auto @min-[680px]:shrink-0">
          <CircleSlash className="size-3.5 shrink-0 text-zinc-500" />
          <span className="text-zinc-500">{t.mcps.inactive} </span>
          <span className="ml-auto font-mono text-[13px] text-zinc-100 @min-[680px]:ml-0">
            {inactiveCount}
          </span>
        </p>
        <span aria-hidden="true" className="hidden h-5 w-px bg-white/10 @min-[680px]:block" />
        <button
          type="button"
          onClick={onAdd}
          className="flex w-full cursor-pointer items-center justify-center gap-1.5 rounded-full border border-emerald-500/25 bg-emerald-500/[0.07] px-3 py-1 text-[11px] font-medium whitespace-nowrap text-zinc-200 transition hover:bg-emerald-500/15 hover:text-emerald-100 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none @min-[680px]:ml-auto @min-[680px]:w-auto @min-[680px]:shrink-0"
          title={t.mcps.addServer}
        >
          <Plus className="size-3.5" />
          {t.mcps.addServer}
        </button>
      </section>

      <div className="mb-4 flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
        <StoreTabs showStore={showStore} onChange={onShowStoreChange} className="mb-0" />

        <div className="flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2">
          <ToolbarSearchInput
            value={query}
            onChange={onQueryChange}
            placeholder={t.common.searchPlaceholderMcps}
            ariaLabel={t.common.searchAriaLabelMcps}
          />
          <SortSelect value={sortMode} onChange={onSortModeChange} />
          <ViewModeToggle view={view} onChange={onViewChange} />
          <button
            type="button"
            onClick={() => onApplyAll(true)}
            disabled={!canBatch}
            className={actionButtonClass}
            title={t.mcps.enableAllTitle(scopeLabel)}
            aria-label={t.mcps.enableAllTitle(scopeLabel)}
          >
            <Power className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={() => onApplyAll(false)}
            disabled={!canBatch}
            className={actionButtonClass}
            title={isGlobal ? t.mcps.disableAllGlobal : t.mcps.disableAllHarness(scopeLabel)}
            aria-label={isGlobal ? t.mcps.disableAllGlobal : t.mcps.disableAllHarness(scopeLabel)}
          >
            {batchPending ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <PowerOff className="size-3.5" />
            )}
          </button>
        </div>
      </div>
    </>
  );
}
