import { useCallback, useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useI18n } from "../hooks/useI18n";
import { cn } from "../lib/utils";

/**
 * Slim frameless-window titlebar: drag region plus minimize /
 * maximize-restore / close. Renders nothing outside Tauri, so the plain
 * browser preview keeps its normal chrome.
 */
export function TitleBar({ isTauri }: { isTauri: boolean }) {
  const { t } = useI18n();
  const [maximized, setMaximized] = useState(false);

  const syncMaximized = useCallback(async () => {
    try {
      setMaximized(await getCurrentWindow().isMaximized());
    } catch {
      // Window API unavailable (permissions, tests): keep last state.
    }
  }, []);

  useEffect(() => {
    if (!isTauri) return;
    void syncMaximized();
    window.addEventListener("focus", syncMaximized);
    return () => window.removeEventListener("focus", syncMaximized);
  }, [isTauri, syncMaximized]);

  if (!isTauri) return null;

  const run = (action: () => Promise<void>) => () => {
    void action().catch(() => undefined);
  };
  const toggleMaximize = run(async () => {
    const win = getCurrentWindow();
    await win.toggleMaximize();
    setMaximized(await win.isMaximized());
  });

  const control =
    "grid w-11 place-items-center self-stretch text-zinc-500 transition outline-none hover:bg-white/10 hover:text-zinc-200 focus-visible:bg-white/10 focus-visible:text-zinc-200";

  return (
    <div className="relative z-30 flex h-9 shrink-0 items-stretch border-b border-white/5 bg-[#0b0b0e]/90 select-none">
      {/* Drag region: branding only; controls live outside it so every click lands. */}
      <div
        data-tauri-drag-region
        className="flex min-w-0 flex-1 items-center gap-2 px-3"
        onDoubleClick={toggleMaximize}
        title="OrquestraAI"
      >
        <span className="truncate font-mono text-[10.5px] tracking-wide text-zinc-600">
          OrquestraAI
        </span>
      </div>
      <div className="flex shrink-0 items-stretch">
        <button
          type="button"
          onClick={run(() => getCurrentWindow().minimize())}
          title={t.common.titlebar.minimize}
          aria-label={t.common.titlebar.minimize}
          className={control}
        >
          <Minus className="size-4" />
        </button>
        <button
          type="button"
          onClick={toggleMaximize}
          title={maximized ? t.common.titlebar.restore : t.common.titlebar.maximize}
          aria-label={maximized ? t.common.titlebar.restore : t.common.titlebar.maximize}
          className={control}
        >
          {maximized ? <Copy className="size-3.5" /> : <Square className="size-3.5" />}
        </button>
        <button
          type="button"
          onClick={run(() => getCurrentWindow().close())}
          title={t.common.titlebar.closeWindow}
          aria-label={t.common.titlebar.closeWindow}
          className={cn(control, "hover:bg-rose-600/90 hover:text-white")}
        >
          <X className="size-4" />
        </button>
      </div>
    </div>
  );
}
