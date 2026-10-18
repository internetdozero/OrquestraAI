import { LayoutGrid, Plus, RefreshCw, Rows3 } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import type { IdeasTab } from "../../hooks/useIdeas";
import { actionButtonClass } from "../ui";
import { cn } from "../../lib/utils";

export type BoardMode = "board" | "list";

interface IdeasHeaderProps {
  loading: boolean;
  activeTab: IdeasTab;
  onTabChange: (tab: IdeasTab) => void;
  /** Board/list switcher; hidden on the blackboard tab. */
  mode: BoardMode;
  onModeChange: (mode: BoardMode) => void;
  onNewIdea: () => void;
  onNewNote: () => void;
  onRefresh: () => void;
}

/**
 * Single top command row: tab pills on the left; view switcher, creation
 * and refresh actions sharing one right-side baseline.
 */
export function IdeasHeader({
  loading,
  activeTab,
  onTabChange,
  mode,
  onModeChange,
  onNewIdea,
  onNewNote,
  onRefresh,
}: IdeasHeaderProps) {
  const { t } = useI18n();
  const s = t.ideas;
  const onIdeas = activeTab === "ideas";

  return (
    <div className="flex flex-wrap items-center gap-2">
      <div
        role="tablist"
        aria-label={s.title}
        className="flex items-center gap-1 rounded-lg border border-white/[0.07] bg-white/[0.02] p-1"
      >
        {(
          [
            { id: "ideas", label: s.tabIdeas },
            { id: "blackboard", label: s.tabBlackboard },
          ] as const
        ).map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={activeTab === tab.id}
            onClick={() => onTabChange(tab.id)}
            className={cn(
              "rounded-md px-3 py-1.5 text-[12px] font-medium transition",
              activeTab === tab.id
                ? "bg-amber-500/15 text-amber-200"
                : "text-zinc-400 hover:text-zinc-200",
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div className="ml-auto flex items-center gap-2">
        {onIdeas && (
          <div
            role="tablist"
            aria-label={s.viewSwitcherAria}
            className="flex items-center gap-1 rounded-lg border border-white/[0.07] bg-white/[0.02] p-1"
          >
            {(
              [
                { id: "board", label: s.boardView, icon: LayoutGrid },
                { id: "list", label: s.listView, icon: Rows3 },
              ] as const
            ).map((option) => {
              const Icon = option.icon;
              return (
                <button
                  key={option.id}
                  type="button"
                  role="tab"
                  aria-selected={mode === option.id}
                  onClick={() => onModeChange(option.id)}
                  className={cn(
                    "flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[12px] font-medium transition",
                    mode === option.id
                      ? "bg-amber-500/15 text-amber-200"
                      : "text-zinc-400 hover:text-zinc-200",
                  )}
                >
                  <Icon className="size-3.5" aria-hidden="true" />
                  {option.label}
                </button>
              );
            })}
          </div>
        )}
        <button
          type="button"
          onClick={onIdeas ? onNewIdea : onNewNote}
          className={cn(
            actionButtonClass,
            "flex items-center gap-1.5 border-amber-500/30 text-amber-200 hover:border-amber-500/50",
          )}
        >
          <Plus className="size-3.5" aria-hidden="true" />
          {onIdeas ? s.newIdea : s.newNote}
        </button>
        <button
          type="button"
          onClick={onRefresh}
          disabled={loading}
          title={t.common.refreshSkills}
          aria-label={t.common.refreshSkills}
          className="grid size-8.5 shrink-0 place-items-center rounded-lg border border-white/10 bg-white/[0.02] text-zinc-400 transition hover:text-zinc-200 disabled:opacity-60"
        >
          <RefreshCw className={cn("size-4", loading && "animate-spin")} />
        </button>
      </div>
    </div>
  );
}
