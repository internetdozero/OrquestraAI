import { useEffect, useRef } from "react";
import { FileText, Orbit, Plug, RefreshCw, Search } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import type { ActiveView } from "../lib/views";
import { cn } from "../lib/utils";

interface HeaderProps {
  query: string;
  onQueryChange: (value: string) => void;
  activeCount: number;
  totalCount: number;
  onRefresh: () => void;
  refreshing: boolean;
  /** The platform view being shown; the skills view owns its harness scope. */
  mode?: ActiveView;
}

export function Header({
  query,
  onQueryChange,
  activeCount,
  totalCount,
  onRefresh,
  refreshing,
  mode = "skills",
}: HeaderProps) {
  const { t } = useI18n();
  const searchRef = useRef<HTMLInputElement>(null);
  const showSkills = mode === "skills";

  useEffect(() => {
    if (!showSkills) return;

    const onKeyDown = (event: KeyboardEvent) => {
      const isShortcut = (event.metaKey || event.ctrlKey) && event.key === "k";
      if (!isShortcut) return;

      event.preventDefault();
      searchRef.current?.focus();
      searchRef.current?.select();
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [showSkills]);

  return (
    <header className="relative z-20 shrink-0 border-b border-white/5 bg-[#0b0b0e]/85 backdrop-blur-xl">
      <div className="flex h-16 items-center gap-3 px-4 sm:gap-4 sm:px-6">
        {/* Brand */}
        <div className="flex min-w-0 items-center gap-3">
          <div className="relative grid size-9 shrink-0 place-items-center rounded-xl bg-gradient-to-br from-emerald-400/20 via-emerald-500/10 to-cyan-500/10 ring-1 ring-emerald-400/25">
            <div className="absolute inset-0 rounded-xl bg-emerald-500/10 blur-md" />
            <Orbit className="relative size-[18px] text-emerald-300" />
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <h1 className="truncate text-[15px] font-semibold tracking-tight text-zinc-100">
                Orquestra<span className="text-emerald-400">AI</span>
              </h1>
              <span className="hidden rounded border border-white/10 bg-white/[0.03] px-1.5 py-px font-mono text-[10px] text-zinc-500 sm:inline">
                v0.1.0
              </span>
            </div>
            <p className="hidden truncate text-[11px] leading-tight text-zinc-500 sm:block">
              {t.common.tagline}
            </p>
          </div>
        </div>

        {showSkills ? (
          <>
            {/* Search */}
            <div className="relative mx-auto w-full max-w-md flex-1">
              <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-zinc-500" />
              <input
                ref={searchRef}
                type="search"
                value={query}
                onChange={(event) => onQueryChange(event.target.value)}
                placeholder={t.common.searchPlaceholder}
                aria-label={t.common.searchAriaLabel}
                spellCheck={false}
                className="h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 pr-16 pl-9 text-sm text-zinc-200 placeholder:text-zinc-600 outline-none transition focus:border-emerald-500/40 focus:bg-zinc-900 focus:ring-2 focus:ring-emerald-500/10 [&::-webkit-search-cancel-button]:appearance-none"
              />
              <kbd className="pointer-events-none absolute top-1/2 right-2 hidden -translate-y-1/2 items-center gap-0.5 rounded border border-white/10 bg-white/[0.04] px-1.5 py-0.5 font-mono text-[10px] text-zinc-500 sm:flex">
                Ctrl&nbsp;K
              </kbd>
            </div>

            {/* Active skills badge + refresh */}
            <div className="flex shrink-0 items-center gap-2">
              <div className="hidden items-center gap-2 rounded-lg border border-emerald-500/25 bg-emerald-500/[0.08] px-3 py-2 md:flex">
                <span className="relative flex size-2">
                  <span className="absolute inline-flex size-full animate-ping rounded-full bg-emerald-400 opacity-60" />
                  <span className="relative inline-flex size-2 rounded-full bg-emerald-400" />
                </span>
                <span className="text-xs font-medium text-emerald-300">
                  {t.common.activeSkills}
                </span>
                <span className="font-mono text-xs text-emerald-200/70">
                  {activeCount}/{totalCount}
                </span>
              </div>

              <button
                type="button"
                onClick={onRefresh}
                disabled={refreshing}
                title={t.common.rescanVault}
                aria-label={t.common.refreshSkills}
                className="grid size-9 place-items-center rounded-lg border border-white/10 bg-white/[0.02] text-zinc-400 transition hover:border-white/20 hover:text-zinc-200 disabled:cursor-wait disabled:opacity-60 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none"
              >
                <RefreshCw className={cn("size-4", refreshing && "animate-spin")} />
              </button>
            </div>
          </>
        ) : mode === "mcps" ? (
          <>
            <div className="flex-1" />
            <span className="flex shrink-0 items-center gap-1.5 rounded-lg border border-white/[0.07] bg-white/[0.02] px-2.5 py-1.5 font-mono text-[10.5px] text-zinc-500">
              <Plug className="size-3 text-emerald-400/80" />
              {t.mcps.headerChip}
            </span>
          </>
        ) : (
          <>
            <div className="flex-1" />
            <span className="flex shrink-0 items-center gap-1.5 rounded-lg border border-white/[0.07] bg-white/[0.02] px-2.5 py-1.5 font-mono text-[10.5px] text-zinc-500">
              <FileText className="size-3 text-emerald-400/80" />
              {t.common.agentsChip}
            </span>
          </>
        )}
      </div>

      <div className="hairline-gradient h-px w-full opacity-60" />
    </header>
  );
}
