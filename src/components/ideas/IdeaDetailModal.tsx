import { useEffect } from "react";
import { createPortal } from "react-dom";
import { ArrowRight, X } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { nextStatusOf } from "../../hooks/useIdeas";
import type { IdeaItem } from "../../lib/ideas";
import { cn } from "../../lib/utils";

interface IdeaDetailModalProps {
  idea: IdeaItem;
  onAdvance: (idea: IdeaItem) => void;
  onEdit: (idea: IdeaItem) => void;
  onDelete: (idea: IdeaItem) => void;
  onClose: () => void;
}

function formatStamp(raw: string): string {
  const date = new Date(raw.replace(" ", "T") + "Z");
  return Number.isNaN(date.getTime()) ? raw : date.toLocaleString();
}

/** Full idea viewer: untruncated body, metadata and quick actions. */
export function IdeaDetailModal({ idea, onAdvance, onEdit, onDelete, onClose }: IdeaDetailModalProps) {
  const { t } = useI18n();
  const s = t.ideas;
  const next = nextStatusOf(idea.status);

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
      aria-label={idea.title}
      className="fixed inset-0 z-[100] flex items-start justify-center overflow-y-auto bg-black/70 px-4 py-10 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="w-full max-w-2xl overflow-hidden rounded-xl border border-white/[0.08] bg-[#0d0d10] shadow-[0_30px_80px_-30px_rgba(0,0,0,0.9)]">
        <div className="flex items-start justify-between gap-4 border-b border-white/[0.06] px-5 py-4">
          <div className="min-w-0">
            <p className="font-mono text-[10.5px] text-zinc-500">
              {idea.project} · #{idea.id}
            </p>
            <h2 className="mt-0.5 text-[15px] font-semibold tracking-tight text-zinc-100">
              {idea.title}
            </h2>
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

        <div className="max-h-[60vh] space-y-4 overflow-y-auto px-5 py-4">
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="rounded border border-white/10 bg-white/[0.04] px-2 py-0.5 font-mono text-[11px] text-zinc-300">
              {idea.project}
            </span>
            <span className="rounded border border-emerald-500/25 bg-emerald-500/10 px-2 py-0.5 text-[11px] font-medium text-emerald-300">
              {s.categories[idea.category as keyof typeof s.categories] ?? idea.category}
            </span>
            <span className="rounded border border-amber-500/25 bg-amber-500/10 px-2 py-0.5 text-[11px] font-medium text-amber-300">
              {s.priorities[idea.priority as keyof typeof s.priorities] ?? idea.priority}
            </span>
            <span className="rounded border border-cyan-500/25 bg-cyan-500/10 px-2 py-0.5 text-[11px] font-medium text-cyan-300">
              {s.statusNames[idea.status] ?? idea.status}
            </span>
          </div>

          {idea.description ? (
            <p className="text-[13px] leading-relaxed whitespace-pre-wrap text-zinc-300">
              {idea.description}
            </p>
          ) : null}
          {idea.tags && (
            <p className="font-mono text-[11px] text-zinc-500">{idea.tags}</p>
          )}

          <dl className="grid grid-cols-1 gap-2 rounded-lg border border-white/[0.06] bg-white/[0.02] p-3 text-[11.5px] sm:grid-cols-2">
            <div>
              <dt className="text-zinc-500">{s.detailCreated}</dt>
              <dd className="mt-0.5 font-mono text-zinc-300">{formatStamp(idea.createdAt)}</dd>
            </div>
            <div>
              <dt className="text-zinc-500">{s.detailUpdated}</dt>
              <dd className="mt-0.5 font-mono text-zinc-300">{formatStamp(idea.updatedAt)}</dd>
            </div>
          </dl>
        </div>

        <div className="flex flex-wrap items-center justify-end gap-2 border-t border-white/[0.06] px-5 py-3">
          {next && (
            <button
              type="button"
              onClick={() => {
                onAdvance(idea);
                onClose();
              }}
              className="flex items-center gap-1.5 rounded-lg border border-amber-500/30 bg-amber-500/15 px-3 py-1.5 text-[12px] font-medium text-amber-200 transition hover:bg-amber-500/25"
            >
              {s.moveTo(s.nextStatus[idea.status] ?? next)}
              <ArrowRight className="size-3.5" aria-hidden="true" />
            </button>
          )}
          <button
            type="button"
            onClick={() => {
              onEdit(idea);
              onClose();
            }}
            className={cn(
              "rounded-lg border border-white/10 px-3 py-1.5 text-[12px] font-medium text-zinc-300 transition hover:text-zinc-100",
            )}
          >
            {s.edit}
          </button>
          <button
            type="button"
            onClick={() => {
              onDelete(idea);
              onClose();
            }}
            className="rounded-lg border border-rose-500/30 px-3 py-1.5 text-[12px] font-medium text-rose-300 transition hover:bg-rose-500/10"
          >
            {s.delete}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
