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
import { GLOBAL_SCOPE, HARNESSES, scopeLabel } from "../lib/harnesses";
import { cn, formatTokens } from "../lib/utils";
import { useI18n } from "../hooks/useI18n";
import { CustomDropdown } from "./CustomDropdown";
import { actionButtonClass, StatTile } from "./ui";

/** Order control for the skill grid, sat with the other grid controls. */
function SortSelect({
  value,
  onChange,
}: {
  value: SortMode;
  onChange: (mode: SortMode) => void;
}) {
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
  inheritedCount: number;
  savedTokens: number;
  visibleCount: number;
  sortMode: SortMode;
  onSortModeChange: (mode: SortMode) => void;
  /** Batch buttons are live (bridge attached, list loaded, nothing pending). */
  canBatch: boolean;
  batchPending: boolean;
  onApplyAll: (enabled: boolean) => void;
  onImport: () => void;
}

/**
 * Dashboard head for one scope: the three metric tiles, the scope identity and
 * the wrapping control cluster (sort, result count, bulk projections).
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
  inheritedCount,
  savedTokens,
  visibleCount,
  sortMode,
  onSortModeChange,
  canBatch,
  batchPending,
  onApplyAll,
  onImport,
}: SkillsToolbarProps) {
  const { t } = useI18n();
  const isGlobal = scope === GLOBAL_SCOPE.id;
  const definition = isGlobal
    ? GLOBAL_SCOPE
    : HARNESSES.find((harness) => harness.id === scope);
  const label = definition?.label ?? "Global";
  const ScopeIcon = definition?.icon ?? GLOBAL_SCOPE.icon;

  const description = isGlobal
    ? t.skills.universalBaselineDesc(HARNESSES.length)
    : inheritedCount > 0
      ? t.skills.inheritedDesc(inheritedCount, scopeLabel(scope))
      : t.skills.isolatedDesc(scopeLabel(scope));

  return (
    <>
      <section className="mb-5 grid grid-cols-1 gap-3 sm:grid-cols-3">
        <StatTile
          label={t.skills.activeSkills}
          value={`${activeCount} / ${totalCount}`}
          hint={
            isGlobal
              ? t.skills.projectedBaseline
              : t.skills.effectiveInHarness
          }
          icon={Layers}
          accent
        />
        <StatTile
          label={t.skills.tokensSaved}
          value={`≈ ${formatTokens(savedTokens)}`}
          hint={t.skills.tokensSavedHint}
          icon={Zap}
        />
        <StatTile
          label={t.skills.disabledSkills}
          value={`${disabledCount}`}
          hint={
            isGlobal
              ? t.skills.indexedVault(totalCount)
              : t.skills.lockedByGlobalCount(inheritedCount)
          }
          icon={CircleSlash}
        />
      </section>

      <div className="mb-4 flex flex-wrap items-start justify-between gap-x-4 gap-y-3">
        <div className="flex min-w-0 items-start gap-3">
          <div
            className={cn(
              "grid size-10 shrink-0 place-items-center rounded-xl border",
              isGlobal
                ? "accent-border accent-bg-soft accent-text"
                : "border-white/[0.08] bg-white/[0.03] text-zinc-300",
            )}
          >
            <ScopeIcon className="size-4" />
          </div>
          <div className="min-w-0">
            <p className="text-[10px] font-medium tracking-[0.14em] text-zinc-500 uppercase">
              {t.skills.scopeLabel}
            </p>
            <h2 className="text-base font-semibold tracking-tight text-zinc-100">
              {t.skills.configuring(label)}
            </h2>
            <p className="mt-0.5 max-w-xl text-[11px] leading-relaxed text-zinc-500">
              {description}
            </p>
          </div>
        </div>

        <div className="flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2 sm:flex-none">
          <SortSelect value={sortMode} onChange={onSortModeChange} />
          <span className="font-mono text-[10.5px] whitespace-nowrap text-zinc-500">
            {t.skills.shownCount(visibleCount)}
          </span>
          <button
            type="button"
            onClick={onImport}
            className={actionButtonClass}
            title={t.skills.import.title}
          >
            <FolderDown className="mr-1 inline size-3" />
            {t.skills.importAction}
          </button>
          <button
            type="button"
            onClick={() => onApplyAll(true)}
            disabled={!canBatch}
            className={actionButtonClass}
            title={t.skills.symlinkAllInto(label)}
          >
            <Power className="mr-1 inline size-3" />
            {t.skills.enableAll}
          </button>
          <button
            type="button"
            onClick={() => onApplyAll(false)}
            disabled={!canBatch}
            className={actionButtonClass}
            title={
              isGlobal
                ? t.skills.unlinkAllGlobal
                : t.skills.unlinkAllHarness(label)
            }
          >
            {batchPending ? (
              <Loader2 className="mr-1 inline size-3 animate-spin" />
            ) : (
              <PowerOff className="mr-1 inline size-3" />
            )}
            {t.skills.disableAll}
          </button>
        </div>
      </div>
    </>
  );
}
