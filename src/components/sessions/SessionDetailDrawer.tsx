import { useEffect } from "react";
import { createPortal } from "react-dom";
import { Check, Copy, Pin, Trash2, X } from "lucide-react";
import { scopeLabel } from "../../lib/harnesses";
import type { UnifiedSession } from "../../lib/sessions";
import {
  absoluteDate,
  formatBytes,
  formatDateTime,
  harnessColor,
} from "../../lib/sessionFormat";
import { cn } from "../../lib/utils";
import { useI18n } from "../../hooks/useI18n";
import { useClipboard } from "../../hooks/useClipboard";
import { TranscriptFeed } from "./TranscriptFeed";

interface SessionDetailDrawerProps {
  session: UnifiedSession;
  onClose: () => void;
  onTogglePin: () => void;
  onDelete: () => void;
  onToast: (message: string) => void;
}

/** Copy affordance for one metadata value, with a 1.5s checkmark. */
function CopyButton({ value, label, onCopied }: { value: string; label: string; onCopied: () => void }) {
  const { copied, copy } = useClipboard(1500);
  return (
    <button
      type="button"
      title={copied ? "Copied!" : label}
      aria-label={copied ? "Copied!" : label}
      onClick={async () => {
        const ok = await copy(value);
        if (ok) onCopied();
      }}
      className={cn(
        "grid size-5 shrink-0 place-items-center rounded transition",
        copied
          ? "text-emerald-400"
          : "text-zinc-600 hover:text-emerald-300",
      )}
    >
      {copied ? <Check className="size-3 text-emerald-400" /> : <Copy className="size-3" />}
    </button>
  );
}

/**
 * Sliding inspector for one session: full metadata, the real transcript
 * feed, and pin/delete actions.
 */
export function SessionDetailDrawer({
  session,
  onClose,
  onTogglePin,
  onDelete,
  onToast,
}: SessionDetailDrawerProps) {
  const { t, locale } = useI18n();
  const s = t.sessions;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  if (typeof document === "undefined") return null;

  const rows: { term: string; value: string; copy?: { value: string; label: string } }[] = [
    {
      term: s.fullId,
      value: session.id,
      copy: { value: session.id, label: s.copyId },
    },
    {
      term: s.workDir,
      value: session.projectPath ?? "—",
      copy: session.projectPath
        ? { value: session.projectPath, label: s.copyPath }
        : undefined,
    },
    { term: s.turns, value: `${session.turnsCount}` },
    { term: s.diskSize, value: formatBytes(session.sizeBytes) },
    {
      term: s.createdAt,
      value: session.createdAt ? formatDateTime(session.createdAt) : "—",
    },
    { term: s.updatedAt, value: formatDateTime(session.updatedAt) },
  ];

  return createPortal(
    <div className="fixed inset-0 z-[100] flex justify-end">
      <button
        type="button"
        aria-label={t.common.close}
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-black/50 backdrop-blur-sm"
      />

      <aside
        role="dialog"
        aria-label={session.title || session.id}
        className="relative flex h-full w-full max-w-md flex-col border-l border-white/[0.08] bg-[#0b0b0e]/95 shadow-[0_0_60px_-10px_rgba(0,0,0,0.9)]"
      >
        <header className="flex items-start gap-3 border-b border-white/[0.06] px-4 py-3.5">
          <div className="min-w-0 flex-1">
            <p className="text-[9.5px] font-medium tracking-[0.14em] text-zinc-600 uppercase">
              {s.drawerMeta}
            </p>
            <h2 className="truncate text-[13px] font-medium text-zinc-100" title={session.title || session.id}>
              {session.title || session.id}
            </h2>
            <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1">
              <span
                className={cn(
                  "rounded-md border px-1.5 py-0.5 font-mono text-[10px] font-medium",
                  harnessColor(session.harness),
                )}
              >
                {scopeLabel(session.harness)}
              </span>
              {session.isPinned && (
                <span className="flex items-center gap-1 rounded-md border border-amber-500/40 bg-amber-500/10 px-1.5 py-0.5 text-[10px] font-medium text-amber-300">
                  <Pin className="size-2.5 fill-current" />
                  {s.pinned}
                </span>
              )}
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t.common.close}
            className="grid size-7 shrink-0 place-items-center rounded text-zinc-500 transition hover:bg-white/[0.05] hover:text-zinc-200"
          >
            <X className="size-3.5" />
          </button>
        </header>

        <div className="min-h-0 flex-1 space-y-3 overflow-y-auto px-4 py-3.5">
          <section>
            <div className="rounded border border-white/[0.06] bg-black/30 px-2.5 py-1">
              {rows.map((row) => (
                <div
                  key={row.term}
                  className="flex min-w-0 items-center justify-between gap-3 border-t border-white/[0.04] py-1.5 first:border-t-0"
                >
                  <span className="shrink-0 text-[10.5px] text-zinc-500">{row.term}</span>
                  <span className="flex min-w-0 items-center gap-1.5">
                    <span
                      className="selectable-text min-w-0 truncate font-mono text-[10.5px] text-zinc-300"
                      title={
                        row.term === s.createdAt && session.createdAt
                          ? absoluteDate(session.createdAt, locale)
                          : row.term === s.updatedAt
                            ? absoluteDate(session.updatedAt, locale)
                            : row.value
                      }
                    >
                      {row.value}
                    </span>
                    {row.copy && (
                      <CopyButton
                        value={row.copy.value}
                        label={row.copy.label}
                        onCopied={() => onToast(s.copied)}
                      />
                    )}
                  </span>
                </div>
              ))}
            </div>
          </section>

          <section>
            <p className="text-[9.5px] font-medium tracking-[0.12em] text-zinc-600 uppercase">
              {s.transcriptTitle}
            </p>
            <div className="mt-1">
              <TranscriptFeed
                key={`${session.harness}:${session.id}`}
                harness={session.harness}
                sessionId={session.id}
                title={session.title || undefined}
                onToast={onToast}
              />
            </div>
          </section>
        </div>

        <footer className="flex items-center justify-between gap-2 border-t border-white/[0.06] px-4 py-3">
          <button
            type="button"
            onClick={onTogglePin}
            aria-pressed={session.isPinned}
            className={cn(
              "flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-[11px] font-medium transition",
              session.isPinned
                ? "border-amber-500/40 bg-amber-500/10 text-amber-300 hover:bg-amber-500/15"
                : "border-white/10 bg-white/[0.03] text-zinc-300 hover:border-white/20 hover:text-white",
            )}
          >
            <Pin className={cn("size-3", session.isPinned && "fill-current")} />
            {session.isPinned ? s.unpinTitle : s.pinTitle}
          </button>
          <button
            type="button"
            onClick={() => {
              if (window.confirm(s.deleteConfirm(session.title || session.id))) onDelete();
            }}
            className="flex items-center gap-1.5 rounded-md border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11px] font-medium text-zinc-400 transition hover:border-rose-500/40 hover:text-rose-300"
          >
            <Trash2 className="size-3" />
            {s.deleteTitle}
          </button>
        </footer>
      </aside>
    </div>,
    document.body,
  );
}
