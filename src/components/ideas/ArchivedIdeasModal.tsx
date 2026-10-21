import { useEffect } from "react";
import { createPortal } from "react-dom";
import { Archive, ArchiveRestore, Trash2, X } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import type { IdeaItem } from "../../lib/ideas";

interface ArchivedIdeasModalProps {
  ideas: IdeaItem[];
  onClose: () => void;
  onOpen: (idea: IdeaItem) => void;
  onRestore: (id: number) => void;
  onDelete: (idea: IdeaItem) => void;
}

function formatDay(raw: string): string {
  const date = new Date(raw.replace(" ", "T") + "Z");
  return Number.isNaN(date.getTime()) ? raw : date.toLocaleDateString();
}

/**
 * Dedicated archive bin: archived ideas leave the board and list views and
 * wait here for inspection, restoration to the backlog, or deletion.
 */
export function ArchivedIdeasModal({
  ideas,
  onClose,
  onOpen,
  onRestore,
  onDelete,
}: ArchivedIdeasModalProps) {
  const { t } = useI18n();
  const s = t.ideas;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={s.archivedModalTitle}
      className="fixed inset-0 z-[100] flex items-start justify-center overflow-y-auto bg-black/70 px-4 py-10 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="w-full max-w-2xl overflow-hidden rounded-xl border border-white/[0.08] bg-[#0d0d10] shadow-[0_30px_80px_-30px_rgba(0,0,0,0.9)]">
        <div className="flex items-center justify-between gap-4 border-b border-white/[0.06] px-5 py-4">
          <div className="flex min-w-0 items-center gap-2.5">
            <span className="grid size-8 shrink-0 place-items-center rounded-lg border border-amber-500/25 bg-amber-500/10">
              <Archive className="size-4 text-amber-300" aria-hidden="true" />
            </span>
            <h2 className="truncate text-[15px] font-semibold tracking-tight text-zinc-100">
              {s.archivedModalTitle}
            </h2>
            <span className="rounded-full bg-white/[0.06] px-2 py-0.5 font-mono text-[10px] font-semibold text-zinc-300 tabular-nums">
              {ideas.length}
            </span>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={s.detailClose}
            className="grid size-8 shrink-0 place-items-center rounded-lg border border-white/[0.07] text-zinc-500 transition hover:text-zinc-200"
          >
            <X className="size-4" />
          </button>
        </div>

        <div className="max-h-[65vh] space-y-2.5 overflow-y-auto p-4">
          {ideas.length === 0 ? (
            <div className="flex flex-col items-center gap-2 rounded-lg border border-dashed border-white/10 px-6 py-12 text-center">
              <Archive className="size-5 text-zinc-600" aria-hidden="true" />
              <p className="text-[13px] font-medium text-zinc-300">{s.archivedEmpty}</p>
              <p className="max-w-xs text-[11.5px] leading-relaxed text-zinc-500">
                {s.archivedEmptyHint}
              </p>
            </div>
          ) : (
            <ol className="space-y-2.5">
              {ideas.map((idea) => (
                <li
                  key={idea.id}
                  className="rounded-lg border border-white/[0.06] bg-white/[0.02] p-3 transition hover:border-white/[0.12]"
                >
                  <div className="flex items-start justify-between gap-3">
                    <button
                      type="button"
                      onClick={() => onOpen(idea)}
                      className="min-w-0 flex-1 text-left"
                    >
                      <p className="font-mono text-[10.5px] text-zinc-500">
                        {idea.project} · #{idea.id}
                      </p>
                      <p className="mt-0.5 truncate text-[13px] font-medium text-zinc-100 hover:text-amber-200">
                        {idea.title}
                      </p>
                      <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
                        <span className="rounded border border-emerald-500/25 bg-emerald-500/10 px-1.5 py-px text-[10.5px] font-medium text-emerald-300">
                          {s.categories[idea.category as keyof typeof s.categories] ??
                            idea.category}
                        </span>
                        <span className="rounded border border-amber-500/25 bg-amber-500/10 px-1.5 py-px text-[10.5px] font-medium text-amber-300">
                          {s.priorities[idea.priority as keyof typeof s.priorities] ??
                            idea.priority}
                        </span>
                        <span className="font-mono text-[10.5px] text-zinc-500 tabular-nums">
                          {formatDay(idea.updatedAt)}
                        </span>
                      </div>
                    </button>
                    <div className="flex shrink-0 items-center gap-1.5">
                      <button
                        type="button"
                        onClick={() => onRestore(idea.id)}
                        title={s.restoreIdea}
                        aria-label={`${s.restoreIdea}: ${idea.title}`}
                        className="grid size-7 place-items-center rounded-md border border-emerald-500/30 text-emerald-300 transition hover:bg-emerald-500/10"
                      >
                        <ArchiveRestore className="size-3.5" />
                      </button>
                      <button
                        type="button"
                        onClick={() => onDelete(idea)}
                        title={s.delete}
                        aria-label={`${s.delete}: ${idea.title}`}
                        className="grid size-7 place-items-center rounded-md border border-white/10 text-zinc-500 transition hover:border-rose-500/40 hover:text-rose-300"
                      >
                        <Trash2 className="size-3.5" />
                      </button>
                    </div>
                  </div>
                </li>
              ))}
            </ol>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
}
