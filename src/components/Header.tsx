import { useEffect, useRef, useState } from "react";
import { FileText, History, Orbit, RefreshCw, Search } from "lucide-react";
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

/** Thin-stroke frameless window glyphs (VS Code style), drawn crisp at 12px. */
function MinimizeGlyph({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 12 12"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.2}
      strokeLinecap="round"
      className={className}
      aria-hidden
    >
      <line x1="2.5" y1="6" x2="9.5" y2="6" />
    </svg>
  );
}

function MaximizeGlyph({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 12 12"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.2}
      className={className}
      aria-hidden
    >
      <rect x="2.5" y="2.5" width="7" height="7" rx="0.5" />
    </svg>
  );
}

function RestoreGlyph({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 12 12"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.2}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden
    >
      <path d="M4.5 2.5h5v5" />
      <rect x="2.5" y="4.5" width="5" height="5" rx="0.5" />
    </svg>
  );
}

function CloseGlyph({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 12 12"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.2}
      strokeLinecap="round"
      className={className}
      aria-hidden
    >
      <path d="M3 3l6 6M9 3L3 9" />
    </svg>
  );
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

  const glyph = "size-3";

  const windowControls = isTauri ? (
    <div className="flex shrink-0 items-center gap-0.5">
      <button
        type="button"
        onClick={runWindow(() => getCurrentWindow().minimize())}
        title={t.common.titlebar.minimize}
        aria-label={t.common.titlebar.minimize}                      className={windowControl}
                    >
                      <MinimizeGlyph className={glyph} />
      </button>
      <button
        type="button"
        onClick={toggleMaximize}
        title={maximized ? t.common.titlebar.restore : t.common.titlebar.maximize}
        aria-label={maximized ? t.common.titlebar.restore : t.common.titlebar.maximize}
        className={windowControl}
      >
        {maximized ? <RestoreGlyph className={glyph} /> : <MaximizeGlyph className={glyph} />}
      </button>
      <button
        type="button"
        onClick={runWindow(() => getCurrentWindow().close())}
        title={t.common.titlebar.closeWindow}
        aria-label={t.common.titlebar.closeWindow}                      className={cn(windowControl, "hover:bg-rose-500/80 hover:text-white")}
                    >
                      <CloseGlyph className={glyph} />
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
      <div className="flex h-11 items-center gap-3 pl-4 pr-2 sm:gap-4 sm:pl-6 sm:pr-2">
        {/* Brand (doubles as a drag handle in the desktop shell) */}
        <div
          className="flex min-w-0 items-center gap-2.5"
          onDoubleClick={isTauri ? toggleMaximize : undefined}
        >
          <div className="accent-bg-soft accent-border relative grid size-7 shrink-0 place-items-center rounded-lg border">
            <div className="accent-bg-soft absolute inset-0 rounded-lg blur-md" />
            <Orbit className="accent-text relative size-4" />
          </div>
          <div className="flex min-w-0 items-center gap-1.5">
            <h1 className="truncate text-[13px] font-semibold tracking-tight text-zinc-100">
              Orquestra<span className="accent-text">AI</span>
            </h1>
            <span className="hidden rounded border border-white/10 bg-white/[0.03] px-1 font-mono text-[9px] text-zinc-500 sm:inline">
              v0.1.0
            </span>
          </div>
        </div>

        {showSearch ? (
          <>
            {/* Search: pinned to the true window center, clear of both flanks. */}
            <div className="absolute top-1/2 left-1/2 w-[min(28rem,calc(100%-36rem))] -translate-x-1/2 -translate-y-1/2">
              <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-zinc-500" />
              <input
                ref={searchRef}
                type="search"
                value={query}
                onChange={(event) => onQueryChange(event.target.value)}
                placeholder={
                  mode === "mcps" ? t.common.searchPlaceholderMcps : t.common.searchPlaceholder
                }
                aria-label={
                  mode === "mcps" ? t.common.searchAriaLabelMcps : t.common.searchAriaLabel
                }
                spellCheck={false}
                className="h-8 w-full rounded-lg border border-white/10 bg-zinc-900/60 pr-14 pl-8 text-[13px] leading-8 text-zinc-200 placeholder:text-zinc-600 outline-none transition focus:border-emerald-500/40 focus:bg-zinc-900 focus:ring-2 focus:ring-emerald-500/10 [&::-webkit-search-cancel-button]:appearance-none"
              />
              <kbd className="pointer-events-none absolute top-1/2 right-1.5 hidden -translate-y-1/2 items-center gap-0.5 rounded border border-white/10 bg-white/[0.04] px-1 font-mono text-[9px] text-zinc-500 sm:flex">
                Ctrl&nbsp;K
              </kbd>
            </div>

            {/* Active skills badge + refresh + window controls */}
            <div className="ml-auto flex shrink-0 items-center gap-2">
              <div className="accent-bg-soft accent-border hidden items-center gap-1.5 rounded-lg border px-2.5 py-1 md:flex">
                <span className="relative flex size-1.5">
                  <span className="accent-dot relative inline-flex size-1.5 rounded-full" />
                </span>
                <span className="accent-text text-[11px] font-medium">{t.common.activeSkills}</span>
                <span className="accent-text font-mono text-[11px] opacity-70">
                  {activeCount}/{totalCount}
                </span>
              </div>

              <button
                type="button"
                onClick={onRefresh}
                disabled={refreshing}
                title={t.common.rescanVault}
                aria-label={t.common.refreshSkills}
                className="grid size-8 place-items-center rounded-lg border border-white/10 bg-white/[0.02] text-zinc-400 transition hover:border-white/20 hover:text-zinc-200 disabled:cursor-wait disabled:opacity-60 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none"
              >
                <RefreshCw className={cn("size-3.5", refreshing && "animate-spin")} />
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
