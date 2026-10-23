import { useEffect, useRef, useState } from "react";
import {
  Copy,
  FileText,
  History,
  Minus,
  Orbit,
  RefreshCw,
  Search,
  Square,
  X,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
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
  /** True inside the desktop shell: fuses frameless window controls into the header. */
  isTauri: boolean;
}

export function Header({
  query,
  onQueryChange,
  activeCount,
  totalCount,
  onRefresh,
  refreshing,
  mode = "skills",
  isTauri,
}: HeaderProps) {
  const { t } = useI18n();
  const searchRef = useRef<HTMLInputElement>(null);
  const showSearch = mode === "skills" || mode === "mcps";
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri) return;
    let active = true;
    const sync = async () => {
      try {
        if (active) setMaximized(await getCurrentWindow().isMaximized());
      } catch {
        // Window API unavailable (permissions, tests): keep last state.
      }
    };
    void sync();
    const onChange = () => void sync();
    window.addEventListener("focus", onChange);
    window.addEventListener("resize", onChange);
    return () => {
      active = false;
      window.removeEventListener("focus", onChange);
      window.removeEventListener("resize", onChange);
    };
  }, [isTauri]);

  const runWindow = (action: () => Promise<void>) => () => {
    void action().catch(() => undefined);
  };
  const toggleMaximize = runWindow(async () => {
    const win = getCurrentWindow();
    await win.toggleMaximize();
    setMaximized(await win.isMaximized());
  });

  const windowControl =
    "grid h-7 w-8 shrink-0 place-items-center rounded-md text-zinc-500 transition outline-none hover:bg-white/10 hover:text-zinc-200 focus-visible:bg-white/10 focus-visible:text-zinc-200 focus-visible:ring-2 focus-visible:ring-emerald-500/40";

  const windowControls = isTauri ? (
    <div className="flex shrink-0 items-center gap-0.5">
      <button
        type="button"
        onClick={runWindow(() => getCurrentWindow().minimize())}
        title={t.common.titlebar.minimize}
        aria-label={t.common.titlebar.minimize}                      className={windowControl}
                    >
                      <Minus className="size-3.5" />
      </button>
      <button
        type="button"
        onClick={toggleMaximize}
        title={maximized ? t.common.titlebar.restore : t.common.titlebar.maximize}
        aria-label={maximized ? t.common.titlebar.restore : t.common.titlebar.maximize}
        className={windowControl}
      >
        {maximized ? <Copy className="size-3" /> : <Square className="size-3" />}
      </button>
      <button
        type="button"
        onClick={runWindow(() => getCurrentWindow().close())}
        title={t.common.titlebar.closeWindow}
        aria-label={t.common.titlebar.closeWindow}                      className={cn(windowControl, "hover:bg-rose-500/80 hover:text-white")}
                    >
                      <X className="size-3.5" />
      </button>
    </div>
  ) : null;

  useEffect(() => {
    if (!showSearch) return;

    const onKeyDown = (event: KeyboardEvent) => {
      const isShortcut = (event.metaKey || event.ctrlKey) && event.key === "k";
      if (!isShortcut) return;

      event.preventDefault();
      searchRef.current?.focus();
      searchRef.current?.select();
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [showSearch]);

  return (
    <header
      className="relative z-20 shrink-0 border-b border-white/5 bg-[#0b0b0e]/85 backdrop-blur-xl"
      onMouseDown={(event) => {
        if (!isTauri || event.button !== 0) return;
        const target = event.target as HTMLElement | null;
        if (target?.closest("button, input, a, select, textarea")) return;
        void getCurrentWindow().startDragging().catch(() => undefined);
      }}
    >
      <div className="flex h-12 items-center gap-3 px-4 sm:gap-4 sm:px-6">
        {/* Brand (doubles as a drag handle in the desktop shell) */}
        <div
          className="flex min-w-0 items-center gap-3"
          onDoubleClick={isTauri ? toggleMaximize : undefined}
        >
          <div className="accent-bg-soft accent-border relative grid size-9 shrink-0 place-items-center rounded-xl border">
            <div className="accent-bg-soft absolute inset-0 rounded-xl blur-md" />
            <Orbit className="accent-text relative size-[18px]" />
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <h1 className="truncate text-[15px] font-semibold tracking-tight text-zinc-100">
                Orquestra<span className="accent-text">AI</span>
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

        {showSearch ? (
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

            {/* Active skills badge + refresh + window controls */}
            <div className="flex shrink-0 items-center gap-2">
              <div className="accent-bg-soft accent-border hidden items-center gap-2 rounded-lg border px-3 py-2 md:flex">
                <span className="relative flex size-2">
                  <span className="accent-dot relative inline-flex size-2 rounded-full" />
                </span>
                <span className="accent-text text-xs font-medium">{t.common.activeSkills}</span>
                <span className="accent-text font-mono text-xs opacity-70">
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
              {isTauri && (
                <>
                  <span aria-hidden className="mx-0.5 h-4 w-px bg-white/10" />
                  {windowControls}
                </>
              )}
            </div>
          </>
        ) : mode === "sessions" ? (
          <>
            <div className="flex-1 self-stretch" />
            <span className="flex shrink-0 items-center gap-1.5 rounded-lg border border-white/[0.07] bg-white/[0.02] px-2.5 py-1.5 font-mono text-[10.5px] text-zinc-500">
              <History className="size-3 text-emerald-400/80" />
              {t.sessions.title}
            </span>
            {isTauri && (
              <>
                <span aria-hidden className="mx-0.5 h-4 w-px bg-white/10" />
                {windowControls}
              </>
            )}
          </>
        ) : (
          <>
            <div className="flex-1 self-stretch" />
            <span className="flex shrink-0 items-center gap-1.5 rounded-lg border border-white/[0.07] bg-white/[0.02] px-2.5 py-1.5 font-mono text-[10.5px] text-zinc-500">
              <FileText className="size-3 text-emerald-400/80" />
              {t.common.agentsChip}
            </span>
            {isTauri && (
              <>
                <span aria-hidden className="mx-0.5 h-4 w-px bg-white/10" />
                {windowControls}
              </>
            )}
          </>
        )}
      </div>

      <div className="hairline-gradient h-px w-full opacity-60" />
    </header>
  );
}
