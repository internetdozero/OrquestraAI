import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { ChevronDown, X } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import {
  commandLine,
  executionKind,
  isSecretKey,
  type McpServer,
} from "../lib/mcps";
import { cn } from "../lib/utils";
import { KindPill } from "./McpCard";

interface McpDetailDrawerProps {
  server: McpServer;
  scopeLabel: string;
  onClose: () => void;
  onEdit: (server: McpServer) => void;
}

/** One environment entry, with secret values masked until revealed. */
function EnvRow({ name, value }: { name: string; value: string }) {
  const { t } = useI18n();
  const secret = isSecretKey(name);
  const [revealed, setRevealed] = useState(false);
  const hidden = secret && !revealed;

  return (
    <div className="flex min-w-0 items-start justify-between gap-3 border-t border-white/[0.04] py-1.5 first:border-t-0">
      <span className="shrink-0 font-mono text-[10.5px] text-zinc-500">
        {name}
      </span>
      <span
        role={secret ? "button" : undefined}
        tabIndex={secret ? 0 : undefined}
        onClick={() => secret && setRevealed((current) => !current)}
        onKeyDown={(event) => {
          if (!secret) return;
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            setRevealed((current) => !current);
          }
        }}
        title={secret ? t.mcps.revealValue : undefined}
        className={cn(
          "selectable-text min-w-0 truncate font-mono text-[10.5px] text-zinc-300",
          secret && "cursor-pointer decoration-dotted hover:text-emerald-300",
          !secret && "cursor-text",
        )}
      >
        {hidden ? "••••••••••" : value}
      </span>
    </div>
  );
}

/**
 * Sliding inspector for one catalog entry: the exact command, arguments,
 * environment and the scopes it is projected into.
 */
export function McpDetailDrawer({
  server,
  scopeLabel,
  onClose,
  onEdit,
}: McpDetailDrawerProps) {
  const { t } = useI18n();
  const envKeys = Object.keys(server.env).sort();
  const args = server.args;
  const anyActive = server.globalEnabled || server.enabledHarnesses.length > 0;
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  // Portalled to the body: the app shell and `main` both create stacking
  // contexts, so an in-place overlay would slide in under the header.
  return createPortal(
    <div className="fixed inset-0 z-50 flex justify-end">
      <button
        type="button"
        aria-label={t.mcps.closeDetails}
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-black/50 backdrop-blur-sm"
      />

      <aside
        role="dialog"
        aria-label={t.mcps.detailsAria(server.name)}
        className="relative flex h-full w-full max-w-md flex-col border-l border-white/[0.08] bg-[#0b0b0e]/95 shadow-[0_0_60px_-10px_rgba(0,0,0,0.9)]"
      >
        <header className="flex items-start gap-3 border-b border-white/[0.06] px-4 py-3.5">
          <div className="min-w-0 flex-1">
            <p className="text-[9.5px] font-medium tracking-[0.14em] text-zinc-600 uppercase">
              {t.mcps.executionDetails}
            </p>
            <h2
              className="truncate font-mono text-[13px] font-medium text-zinc-100"
              title={server.name}
            >
              {server.name}
            </h2>
            <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1">
              <KindPill kind={executionKind(server.command)} />
              <span className="font-mono text-[9.5px] text-zinc-600">
                {t.mcps.scopeOf(scopeLabel)}
              </span>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t.common.close}
            className="grid size-7 shrink-0 place-items-center rounded text-zinc-500 transition hover:bg-white/[0.05] hover:text-zinc-200 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none"
          >
            <X className="size-3.5" />
          </button>
        </header>

        <div className="min-h-0 flex-1 space-y-3 overflow-y-auto px-4 py-3.5">
          <section>
            <p className="text-[9.5px] font-medium tracking-[0.12em] text-zinc-600 uppercase">
              {t.mcps.command}
            </p>
            <code className="selectable-text mt-1 block overflow-x-auto rounded border border-white/[0.06] bg-black/40 p-2 font-mono text-[10.5px] whitespace-pre text-emerald-200/90">
              {commandLine(server)}
            </code>
          </section>

          <section>
            <p className="text-[9.5px] font-medium tracking-[0.12em] text-zinc-600 uppercase">
              {t.mcps.environment}{" "}
              <span className="text-zinc-700">· {envKeys.length}</span>
            </p>
            <div className="mt-1 rounded border border-white/[0.06] bg-black/30 px-2.5 py-1">
              {envKeys.length === 0 ? (
                <p className="py-1 text-[10.5px] text-zinc-600">
                  {t.mcps.noneDeclared}
                </p>
              ) : (
                envKeys.map((key) => (
                  <EnvRow key={key} name={key} value={server.env[key]} />
                ))
              )}
            </div>
          </section>

          <section>
            <button
              type="button"
              onClick={() => setExpanded((previous) => !previous)}
              aria-expanded={expanded}
              className="flex items-center gap-1.5 rounded text-[9.5px] font-medium tracking-[0.12em] text-zinc-600 uppercase transition hover:text-zinc-400 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none"
            >
              <ChevronDown
                className={cn(
                  "size-3 transition-transform duration-200",
                  expanded && "rotate-180",
                )}
              />
              {t.mcps.arguments} <span className="text-zinc-700">· {args.length}</span>
            </button>
            {expanded && (
              <div className="mt-1 space-y-px rounded border border-white/[0.06] bg-black/30 p-2">
                {args.length === 0 ? (
                  <p className="text-[10.5px] text-zinc-600">{t.mcps.noneDeclared}</p>
                ) : (
                  args.map((arg, index) => (
                    <code
                      key={`${index}-${arg}`}
                      className="block truncate font-mono text-[10.5px] text-zinc-400"
                      title={arg}
                    >
                      <span className="mr-1.5 text-zinc-700">
                        {String(index).padStart(2, "0")}
                      </span>
                      {arg}
                    </code>
                  ))
                )}
              </div>
            )}
          </section>

          <section>
            <p className="text-[9.5px] font-medium tracking-[0.12em] text-zinc-600 uppercase">
              {t.mcps.enabledIn}
            </p>
            <div className="mt-1 flex flex-wrap gap-1">
              {server.globalEnabled && (
                <span className="rounded border border-emerald-500/25 bg-emerald-500/10 px-1.5 py-px font-mono text-[9.5px] text-emerald-300">
                  {t.mcps.globalBadge}
                </span>
              )}
              {server.enabledHarnesses.map((harness) => (
                <span
                  key={harness}
                  className="rounded border border-white/[0.09] bg-white/[0.04] px-1.5 py-px font-mono text-[9.5px] text-zinc-300"
                >
                  {harness}
                </span>
              ))}
              {!anyActive && (
                <span className="font-mono text-[9.5px] text-zinc-600">
                  {t.mcps.nowhereYet}
                </span>
              )}
            </div>
          </section>

          {server.description && (
            <p className="text-[12px] leading-relaxed text-zinc-400">
              {server.description}
            </p>
          )}
        </div>

        <footer className="flex items-center justify-between gap-2 border-t border-white/[0.06] px-4 py-3">
          <span className="font-mono text-[10px] text-zinc-600">
            {server.id}
          </span>
          <button
            type="button"
            onClick={() => onEdit(server)}
            className="rounded-md border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11px] font-medium text-zinc-300 transition hover:border-white/20 hover:text-white focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none"
          >
            {t.mcps.editServer}
          </button>
        </footer>
      </aside>
    </div>,
    document.body,
  );
}
