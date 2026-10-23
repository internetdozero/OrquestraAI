import { useEffect, useState } from "react";
import { Orbit, RefreshCw } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useI18n } from "../hooks/useI18n";
import { cn } from "../lib/utils";

interface HeaderProps {
  onRefresh: () => void;
  refreshing: boolean;
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

export function Header({ onRefresh, refreshing, isTauri }: HeaderProps) {
  const { t } = useI18n();
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

  // Universal rescan: every view reloads through the shell token.
  const refreshButton = (
    <button
      type="button"
      onClick={onRefresh}
      disabled={refreshing}
      title={t.common.reloadView}
      aria-label={t.common.reloadView}
      className="grid size-8 shrink-0 place-items-center rounded-lg border border-white/10 bg-white/[0.02] text-zinc-400 transition hover:border-white/20 hover:text-zinc-200 disabled:cursor-wait disabled:opacity-60 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none"
    >
      <RefreshCw className={cn("size-3.5", refreshing && "animate-spin")} />
    </button>
  );

  const glyph = "size-3";

  const windowControls = isTauri ? (
    <div className="flex shrink-0 items-center gap-0.5">
      <button
        type="button"
        onClick={runWindow(() => getCurrentWindow().minimize())}
        title={t.common.titlebar.minimize}
        aria-label={t.common.titlebar.minimize}
        className={windowControl}
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
        aria-label={t.common.titlebar.closeWindow}
        className={cn(windowControl, "hover:bg-rose-500/80 hover:text-white")}
      >
        <CloseGlyph className={glyph} />
      </button>
    </div>
  ) : null;

  return (
    <header
      className="relative z-20 shrink-0 border-b border-white/5 bg-[#0b0b0e]/85 backdrop-blur-xl"
      onMouseDown={(event) => {
        if (!isTauri || event.button !== 0) return;
        const target = event.target as HTMLElement | null;
        if (target?.closest("button, input, a, select, textarea")) return;
        void getCurrentWindow()
          .startDragging()
          .catch(() => undefined);
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

        <div className="flex-1 self-stretch" />
        {/* Refresh + window controls */}
        <div className="ml-auto flex shrink-0 items-center gap-2">
          {refreshButton}
          {isTauri && (
            <>
              <span aria-hidden className="mx-0.5 h-4 w-px bg-white/10" />
              {windowControls}
            </>
          )}
        </div>
      </div>

      <div className="hairline-gradient h-px w-full opacity-60" />
    </header>
  );
}
