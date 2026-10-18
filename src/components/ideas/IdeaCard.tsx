import { ArrowRight, Boxes, Bug, GripVertical, Lightbulb, Palette, Pencil, Sparkles, Trash2 } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { nextStatusOf } from "../../hooks/useIdeas";
import type { IdeaItem } from "../../lib/ideas";
import { cn } from "../../lib/utils";

const CATEGORY_STYLE: Record<string, { icon: typeof Bug; chip: string }> = {
  feature: { icon: Sparkles, chip: "border-emerald-500/25 bg-emerald-500/10 text-emerald-300" },
  architecture: { icon: Boxes, chip: "border-cyan-500/25 bg-cyan-500/10 text-cyan-300" },
  bug: { icon: Bug, chip: "border-rose-500/25 bg-rose-500/10 text-rose-300" },
  ux: { icon: Palette, chip: "border-violet-500/25 bg-violet-500/10 text-violet-300" },
  idea: { icon: Lightbulb, chip: "border-amber-500/25 bg-amber-500/10 text-amber-300" },
};

const PRIORITY_STYLE: Record<string, string> = {
  critical: "border-red-500/30 bg-red-500/10 text-red-300",
  high: "border-amber-500/30 bg-amber-500/10 text-amber-300",
  medium: "border-blue-500/30 bg-blue-500/10 text-blue-300",
  low: "border-white/10 bg-white/[0.04] text-zinc-400",
};

interface IdeaCardProps {
  idea: IdeaItem;
  /** Compact density for finished work: one-line title, no description. */
  compact?: boolean;
  onAdvance: (idea: IdeaItem) => void;
  onEdit: (idea: IdeaItem) => void;
  onDelete: (idea: IdeaItem) => void;
  onOpen: (idea: IdeaItem) => void;
}

/**
 * Draggable vault card. Clicking anywhere outside the action buttons opens
 * the detail viewer; completed columns render the compact variant.
 */
export function IdeaCard({ idea, compact = false, onAdvance, onEdit, onDelete, onOpen }: IdeaCardProps) {
  const { t } = useI18n();
  const s = t.ideas;
  const next = nextStatusOf(idea.status);
  const category = CATEGORY_STYLE[idea.category] ?? CATEGORY_STYLE.idea;
  const CategoryIcon = category.icon;
  const updated = new Date(idea.updatedAt.replace(" ", "T") + "Z");

  return (
    <article
      draggable
      onDragStart={(event) => {
        event.dataTransfer.setData("text/idea-id", String(idea.id));
        event.dataTransfer.setData("text/idea-status", idea.status);
        event.dataTransfer.effectAllowed = "move";
      }}
      onClick={() => onOpen(idea)}
      onKeyDown={(event) => {
        if (event.key === "Enter") onOpen(idea);
      }}
      tabIndex={0}
      role="button"
      aria-label={idea.title}
      className={cn(
        "group flex cursor-grab flex-col gap-2 rounded-xl border border-white/[0.06] bg-zinc-900/30 transition hover:border-white/[0.12] active:cursor-grabbing",
        compact ? "p-2" : "p-3",
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <h4
          className={cn(
            "min-w-0 flex-1 leading-snug font-medium text-zinc-100",
            compact ? "truncate text-[12px]" : "text-[12.5px]",
          )}
        >
          {idea.title}
        </h4>
        <div className="flex shrink-0 items-center gap-1 opacity-0 transition group-hover:opacity-100 focus-within:opacity-100">
          <GripVertical className="size-3 text-zinc-600" aria-hidden="true" />
          <button
            type="button"
            onClick={(event) => {
              event.stopPropagation();
              onEdit(idea);
            }}
            title={s.edit}
            aria-label={s.edit}
            className="grid size-6.5 place-items-center rounded-md border border-white/10 text-zinc-500 transition hover:text-zinc-200"
          >
            <Pencil className="size-3" />
          </button>
          <button
            type="button"
            onClick={(event) => {
              event.stopPropagation();
              onDelete(idea);
            }}
            title={s.delete}
            aria-label={s.delete}
            className="grid size-6.5 place-items-center rounded-md border border-white/10 text-zinc-500 transition hover:border-rose-500/40 hover:text-rose-300"
          >
            <Trash2 className="size-3" />
          </button>
        </div>
      </div>

      {!compact && idea.description && (
        <p className="line-clamp-3 text-[11.5px] leading-relaxed text-zinc-400">{idea.description}</p>
      )}

      <div className="flex flex-wrap items-center gap-1.5">
        <span className="rounded border border-white/10 bg-white/[0.04] px-1.5 py-0.5 font-mono text-[10px] text-zinc-400">
          {idea.project}
        </span>
        <span
          className={cn(
            "flex items-center gap-1 rounded border px-1.5 py-0.5 text-[10px] font-medium",
            category.chip,
          )}
        >
          <CategoryIcon className="size-2.5" aria-hidden="true" />
          {s.categories[idea.category as keyof typeof s.categories] ?? idea.category}
        </span>
        {!compact && (
          <span
            className={cn(
              "rounded border px-1.5 py-0.5 text-[10px] font-medium",
              PRIORITY_STYLE[idea.priority] ?? PRIORITY_STYLE.low,
            )}
          >
            {s.priorities[idea.priority as keyof typeof s.priorities] ?? idea.priority}
          </span>
        )}
        {!Number.isNaN(updated.getTime()) && (
          <span className="ml-auto text-[10px] text-zinc-600">
            {updated.toLocaleDateString()}
          </span>
        )}
      </div>

      {!compact && next && (
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onAdvance(idea);
          }}
          className="mt-0.5 flex items-center justify-center gap-1 rounded-lg border border-white/[0.07] bg-white/[0.02] px-2 py-1.5 text-[11px] font-medium text-zinc-400 transition hover:border-amber-500/30 hover:text-amber-200"
        >
          {s.moveTo(s.nextStatus[idea.status] ?? next)}
          <ArrowRight className="size-3" aria-hidden="true" />
        </button>
      )}
    </article>
  );
}
