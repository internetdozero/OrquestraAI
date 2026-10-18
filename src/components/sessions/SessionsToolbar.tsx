import { useMemo } from "react";
import { ArrowDownWideNarrow, ListFilter, Pin, Search, TriangleAlert } from "lucide-react";
import { HARNESSES } from "../../lib/harnesses";
import { useI18n } from "../../hooks/useI18n";
import type { SessionFilterState, SessionSort } from "../../hooks/useSessions";
import { cn } from "../../lib/utils";
import { CustomDropdown, type DropdownOption } from "../CustomDropdown";

interface SessionsToolbarProps {
  filter: SessionFilterState;
  onChange: (next: SessionFilterState) => void;
  visibleCount: number;
}

/** Search + harness scope + pin/candidate toggles + sort. */
export function SessionsToolbar({ filter, onChange, visibleCount }: SessionsToolbarProps) {
  const { t } = useI18n();
  const s = t.sessions;
  const sortLabel =
    filter.sort === "newest" ? s.sortNewest : filter.sort === "oldest" ? s.sortOldest : s.sortLargest;

  const harnessOptions = useMemo<DropdownOption<string>[]>(() => [
    { id: "all", label: s.harnessAll },
    ...HARNESSES.map((h) => ({ id: h.id, label: h.label })),
  ], [s.harnessAll]);

  const sortOptions = useMemo<DropdownOption<SessionSort>[]>(() => [
    { id: "newest", label: s.sortNewest },
    { id: "oldest", label: s.sortOldest },
    { id: "largest", label: s.sortLargest },
  ], [s.sortNewest, s.sortOldest, s.sortLargest]);

  return (
    <div className="flex flex-col gap-2 rounded-xl border border-white/[0.06] bg-zinc-900/30 p-3">
      <div className="flex flex-col gap-2 lg:flex-row lg:items-center">
        <div className="relative flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-zinc-500" />
          <input
            type="search"
            value={filter.query}
            onChange={(e) => onChange({ ...filter, query: e.target.value })}
            placeholder={s.searchPlaceholder}
            aria-label={s.searchAria}
            spellCheck={false}
            className="h-8.5 w-full rounded-lg border border-white/10 bg-zinc-900/60 py-1.5 pr-3 pl-8 text-[12.5px] text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-emerald-500/40 focus:ring-2 focus:ring-emerald-500/10 [&::-webkit-search-cancel-button]:appearance-none"
          />
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <CustomDropdown
            value={filter.harness}
            options={harnessOptions}
            onChange={(id) => onChange({ ...filter, harness: id })}
            ariaLabel={s.harnessAll}
            title={s.harnessAll}
            icon={ListFilter}
          />
          <CustomDropdown
            value={filter.sort}
            options={sortOptions}
            onChange={(id) => onChange({ ...filter, sort: id })}
            ariaLabel={s.sortLabel}
            title={s.sortLabel}
            icon={ArrowDownWideNarrow}
          />
          <span className="font-mono text-[10.5px] text-zinc-500">{visibleCount} · {sortLabel}</span>
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          aria-pressed={filter.onlyPinned}
          onClick={() => onChange({ ...filter, onlyPinned: !filter.onlyPinned })}
          className={cn(
            "flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-[11px] font-medium transition",
            filter.onlyPinned
              ? "border-amber-500/40 bg-amber-500/10 text-amber-200"
              : "border-white/10 bg-white/[0.02] text-zinc-400 hover:text-zinc-200",
          )}
        >
          <Pin className="size-3" />
          {s.onlyPinned}
        </button>
        <button
          type="button"
          aria-pressed={filter.purgeCandidates}
          onClick={() => onChange({ ...filter, purgeCandidates: !filter.purgeCandidates })}
          className={cn(
            "flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-[11px] font-medium transition",
            filter.purgeCandidates
              ? "border-rose-500/40 bg-rose-500/10 text-rose-200"
              : "border-white/10 bg-white/[0.02] text-zinc-400 hover:text-zinc-200",
          )}
        >
          <TriangleAlert className="size-3" />
          {s.purgeCandidates}
        </button>
      </div>
    </div>
  );
}
