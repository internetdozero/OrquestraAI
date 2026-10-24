import {
  ArrowUpDown,
  CircleSlash,
  FolderDown,
  Layers,
  Loader2,
  Power,
  PowerOff,
  Zap,
} from "lucide-react";
import type { SortMode } from "../lib/skills";
import { GLOBAL_SCOPE, HARNESSES } from "../lib/harnesses";
import { formatTokens } from "../lib/utils";
import { useI18n } from "../hooks/useI18n";
import { CustomDropdown } from "./CustomDropdown";
import { StoreTabs } from "./StoreTabs";
import { actionButtonClass, ToolbarSearchInput, ViewModeToggle } from "./ui";

/** Order control for the skill grid, sat with the other grid controls. */
function SortSelect({ value, onChange }: { value: SortMode; onChange: (mode: SortMode) => void }) {
  const { t } = useI18n();

  const options: { id: SortMode; label: string }[] = [
    { id: "active", label: t.skills.sortOptions.active },
    { id: "alpha", label: t.skills.sortOptions.alpha },
    { id: "tokensDesc", label: t.skills.sortOptions.tokensDesc },
    { id: "tokensAsc", label: t.skills.sortOptions.tokensAsc },
  ];

  return (
    <CustomDropdown
      value={value}
      options={options}
      onChange={onChange}
      ariaLabel={t.skills.sortSkills}
      title={t.skills.orderGrid}
      icon={ArrowUpDown}
    />
  );
}

interface SkillsToolbarProps {
  /** Harness id, or `global`. */
  scope: string;
  activeCount: number;
  totalCount: number;
  disabledCount: number;
  savedTokens: number;
  sortMode: SortMode;
  onSortModeChange: (mode: SortMode) => void;
  /** Batch buttons are live (bridge attached, list loaded, nothing pending). */
  canBatch: boolean;
  batchPending: boolean;
  onApplyAll: (enabled: boolean) => void;
  onImport: () => void;
  /** Library | Official Store switch, sat with the other grid controls. */
  showStore: boolean;
  onShowStoreChange: (show: boolean) => void;
  /** Live search text owned by the parent view. */
  query: string;
  onQueryChange: (query: string) => void;
  /** Grid | list switch for the skill grid. */
  view: "grid" | "list";
  onViewChange: (view: "grid" | "list") => void;
}

/**
 * Dashboard head for one scope: the three metric tiles and a single control
 * row (Library | Store switch, sort, result count, bulk projections).
 *
 * Everything here is `flex-wrap` with `min-w-0` children: at the 900px minimum
 * window the control cluster drops to its own row instead of squeezing the
 * stat tiles or clipping the sort menu.
 */
export function SkillsToolbar({
  scope,
  activeCount,
  totalCount,
  disabledCount,
  savedTokens,
  sortMode,
  onSortModeChange,
  canBatch,
  batchPending,
  onApplyAll,
  onImport,
  showStore,
  onShowStoreChange,
  query,
  onQueryChange,
  view,
  onViewChange,
}: SkillsToolbarProps) {
  const { t } = useI18n();
  const isGlobal = scope === GLOBAL_SCOPE.id;
  const definition = isGlobal ? GLOBAL_SCOPE : HARNESSES.find((harness) => harness.id === scope);
  const label = definition?.label ?? "Global";

  return (
    <>
      <section
        aria-label={`${t.skills.activeSkills}, ${t.skills.tokensSaved}, ${t.skills.disabledSkills}`}
        className="mb-4 flex min-w-0 flex-col gap-y-2 rounded-xl border border-white/[0.06] bg-zinc-900/30 px-4 py-3 @min-[680px]:flex-row @min-[680px]:flex-wrap @min-[680px]:items-center @min-[680px]:gap-x-4"
      >
        <p className="flex w-full items-center gap-2 text-[11px] font-medium tracking-wider uppercase @min-[680px]:w-auto @min-[680px]:shrink-0">
          <Layers className="size-3.5 shrink-0 text-zinc-500" />
          <span className="text-zinc-500">{t.skills.activeSkills} </span>
          <span className="ml-auto font-mono text-[13px] text-zinc-100 @min-[680px]:ml-0">
            {activeCount} / {totalCount}
          </span>
        </p>
        <p className="flex w-full items-center gap-2 text-[11px] font-medium tracking-wider uppercase @min-[680px]:w-auto @min-[680px]:shrink-0">
          <Zap className="size-3.5 shrink-0 text-zinc-500" />
          <span className="text-zinc-500">{t.skills.tokensSaved} </span>
          <span className="ml-auto font-mono text-[13px] text-zinc-100 @min-[680px]:ml-0">
            ≈ {formatTokens(savedTokens)}
          </span>
        </p>
        <p className="flex w-full items-center gap-2 text-[11px] font-medium tracking-wider uppercase @min-[680px]:w-auto @min-[680px]:shrink-0">
          <CircleSlash className="size-3.5 shrink-0 text-zinc-500" />
          <span className="text-zinc-500">{t.skills.disabledSkills} </span>
          <span className="ml-auto font-mono text-[13px] text-zinc-100 @min-[680px]:ml-0">
            {disabledCount}
          </span>
        </p>
        <span aria-hidden="true" className="hidden h-5 w-px bg-white/10 @min-[680px]:block" />
        <button
          type="button"
          onClick={onImport}
          className="flex w-full cursor-pointer items-center justify-center gap-1.5 rounded-full border border-emerald-500/25 bg-emerald-500/[0.07] px-3 py-1 text-[11px] font-medium whitespace-nowrap text-zinc-200 transition hover:bg-emerald-500/15 hover:text-emerald-100 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none @min-[680px]:ml-auto @min-[680px]:w-auto @min-[680px]:shrink-0"
          title={t.skills.import.title}
        >
          <FolderDown className="size-3.5" />
          {t.skills.importAction}
        </button>
      </section>

      <div className="mb-4 flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
        <StoreTabs showStore={showStore} onChange={onShowStoreChange} className="mb-0" />

        <div className="flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2">
          <ToolbarSearchInput
            value={query}
            onChange={onQueryChange}
            placeholder={t.common.searchPlaceholder}
            ariaLabel={t.common.searchAriaLabel}
          />
          <SortSelect value={sortMode} onChange={onSortModeChange} />
          <ViewModeToggle view={view} onChange={onViewChange} />
          <button
            type="button"
            onClick={() => onApplyAll(true)}
            disabled={!canBatch}
            className={actionButtonClass}
            title={t.skills.symlinkAllInto(label)}
            aria-label={t.skills.symlinkAllInto(label)}
          >
            <Power className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={() => onApplyAll(false)}
            disabled={!canBatch}
            className={actionButtonClass}
            title={isGlobal ? t.skills.unlinkAllGlobal : t.skills.unlinkAllHarness(label)}
            aria-label={isGlobal ? t.skills.unlinkAllGlobal : t.skills.unlinkAllHarness(label)}
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
