import { useState } from "react";
import { ChevronDown, ChevronRight, Inbox } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import type { IdeaItem } from "../../lib/ideas";
import { StatePanel } from "../ui";
import { cn } from "../../lib/utils";
import { IdeaCard } from "./IdeaCard";

interface IdeasBoardProps {
  ideas: IdeaItem[];
  loading: boolean;
  onAdvance: (idea: IdeaItem) => void;
  onEdit: (idea: IdeaItem) => void;
  onDelete: (idea: IdeaItem) => void;
  onOpen: (idea: IdeaItem) => void;
  onDropIdea: (ideaId: number, status: string) => void;
}

const COLUMNS = [
  { status: "open", titleKey: "colOpen" },
  { status: "in_progress", titleKey: "colInProgress" },
  { status: "completed", titleKey: "colCompleted" },
] as const;

/** Completed columns page at this many cards before offering more. */
const COMPLETED_PAGE = 5;

/** localStorage key shielding user-opened columns across remounts/restarts. */
const STORAGE_KEY_USER_OPENED = "orquestra:ideas:user_opened_columns";

/** Best-effort read of the persisted set; denied storage falls back to empty. */
function readUserOpened(): ReadonlySet<string> {
  if (typeof window === "undefined") return new Set();
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY_USER_OPENED);
    if (!raw) return new Set();
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return new Set();
    return new Set(
      parsed.filter((entry): entry is string => typeof entry === "string"),
    );
  } catch {
    return new Set();
  }
}

/**
 * Kanban board: collapsible columns, HTML5 drop targets with highlight, and
 * demand paging for the completed column so it never stretches forever.
 */
export function IdeasBoard({
  ideas,
  loading,
  onAdvance,
  onEdit,
  onDelete,
  onOpen,
  onDropIdea,
}: IdeasBoardProps) {
  const { t } = useI18n();
  const s = t.ideas;
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(
    () => new Set(["completed"]),
  );
  // Columns the user explicitly opened: shields them from the auto-collapse
  // rule for empty columns below. Persisted so remounts keep the choice.
  const [userOpened, setUserOpened] =
    useState<ReadonlySet<string>>(readUserOpened);
  /** Update the set and persist it; storage failure keeps memory state. */
  const persistOpened = (next: ReadonlySet<string>) => {
    setUserOpened(next);
    try {
      window.localStorage.setItem(STORAGE_KEY_USER_OPENED, JSON.stringify([...next]));
    } catch {
      // Persisting the preference is a nicety, never a requirement.
    }
  };
  const [expandedCompleted, setExpandedCompleted] = useState(false);
  const [dropStatus, setDropStatus] = useState<string | null>(null);

  if (!loading && ideas.length === 0) {
    return <StatePanel icon={Inbox} title={s.empty} description={s.emptyHint} />;
  }

  const toggle = (status: string) => {
    // The strip's expand/collapse buttons act on the *effective* state:
    // an auto-collapsed empty column must open on click, not sink into
    // the manual set and stay shut.
    const isEmpty = !ideas.some((idea) => idea.status === status);
    const effectivelyCollapsed =
      collapsed.has(status) || (isEmpty && !userOpened.has(status));
    if (effectivelyCollapsed) {
      setCollapsed((previous) => {
        const next = new Set(previous);
        next.delete(status);
        return next;
      });
      persistOpened(new Set(userOpened).add(status));
    } else {
      setCollapsed((previous) => new Set(previous).add(status));
      const next = new Set(userOpened);
      next.delete(status);
      persistOpened(next);
    }
  };

  /** No-op IPC guard: dropping onto the card's own column changes nothing. */
  const dropIdea = (ideaId: number, sourceStatus: string | undefined, targetStatus: string) => {
    const current = sourceStatus ?? ideas.find((idea) => idea.id === ideaId)?.status;
    if (current === targetStatus) return;
    onDropIdea(ideaId, targetStatus);
  };

  return (
    <div className="grid grid-cols-1 items-start gap-3 sm:grid-cols-2 xl:flex">
      {COLUMNS.map((column) => {
        const rows = ideas.filter((idea) => idea.status === column.status);
        // Empty columns auto-collapse to the slim strip so zero-card
        // statuses cost no horizontal space; an explicit user open wins.
        const isCollapsed =
          collapsed.has(column.status) ||
          (rows.length === 0 && !userOpened.has(column.status));
        const isCompleted = column.status === "completed";
        const shown =
          isCompleted && !expandedCompleted ? rows.slice(0, COMPLETED_PAGE) : rows;
        const hidden = rows.length - shown.length;

        if (isCollapsed) {
          return (
            <section
              key={column.status}
              aria-label={s[column.titleKey]}
              onDragOver={(event) => {
                event.preventDefault();
                event.dataTransfer.dropEffect = "move";
                setDropStatus(column.status);
              }}
              onDragLeave={() =>
                setDropStatus((current) => (current === column.status ? null : current))
              }
              onDrop={(event) => {
                event.preventDefault();
                setDropStatus(null);
                const raw = event.dataTransfer.getData("text/idea-id");
                const id = Number.parseInt(raw, 10);
                if (!Number.isInteger(id)) return;
                dropIdea(
                  id,
                  event.dataTransfer.getData("text/idea-status") || undefined,
                  column.status,
                );
              }}
              className={cn(
                "w-full overflow-hidden rounded-xl border bg-black/20 hover:border-white/[0.12] hover:bg-white/[0.02] xl:w-11 xl:shrink-0",
                dropStatus === column.status
                  ? "border-amber-500/50 bg-amber-500/[0.04]"
                  : "border-white/[0.05]",
              )}
            >
              <button
                type="button"
                onClick={() => toggle(column.status)}
                title={s.expandColumn}
                aria-label={`${s.expandColumn}: ${s[column.titleKey]}`}
                aria-expanded={false}
                className="flex h-full min-h-64 w-full cursor-pointer flex-col items-center gap-3 py-3 select-none"
              >
                <ChevronRight className="size-4 shrink-0 text-zinc-400" aria-hidden="true" />
                <span className="pointer-events-none rounded-full bg-white/[0.06] px-2 py-0.5 font-mono text-[10px] font-semibold text-zinc-300 tabular-nums select-none">
                  {rows.length}
                </span>
                <span
                  aria-hidden="true"
                  className="pointer-events-none mt-4 text-[11px] font-semibold tracking-[0.18em] whitespace-nowrap text-zinc-400 uppercase select-none"
                  style={{ writingMode: "vertical-rl", transform: "rotate(180deg)" }}
                >
                  {s[column.titleKey]}
                </span>
              </button>
            </section>
          );
        }

        return (
          <section
            key={column.status}
            aria-label={s[column.titleKey]}
            onDragOver={(event) => {
              event.preventDefault();
              event.dataTransfer.dropEffect = "move";
              setDropStatus(column.status);
            }}
            onDragLeave={() =>
              setDropStatus((current) => (current === column.status ? null : current))
            }
            onDrop={(event) => {
              event.preventDefault();
              setDropStatus(null);
              const raw = event.dataTransfer.getData("text/idea-id");
              const id = Number.parseInt(raw, 10);
              if (!Number.isInteger(id)) return;
              dropIdea(
                id,
                event.dataTransfer.getData("text/idea-status") || undefined,
                column.status,
              );
            }}
            className={cn(
              "flex min-w-0 flex-col gap-2 rounded-xl border bg-black/20 p-2.5 transition-colors xl:min-w-0 xl:flex-1",
              dropStatus === column.status
                ? "border-amber-500/50 bg-amber-500/[0.04]"
                : "border-white/[0.05]",
            )}
          >
            <header className="flex items-center justify-between px-1">
              <button
                type="button"
                onClick={() => toggle(column.status)}
                title={s.collapseColumn}
                aria-label={`${s.collapseColumn}: ${s[column.titleKey]}`}
                aria-expanded
                className="flex min-w-0 items-center gap-1.5 rounded-md px-1 py-0.5 text-left transition hover:bg-white/[0.04]"
              >
                <ChevronDown className="size-3.5 shrink-0 text-zinc-500" aria-hidden="true" />
                <h3 className="truncate text-[11px] font-semibold tracking-[0.1em] text-zinc-400 uppercase">
                  {s[column.titleKey]}
                </h3>
              </button>
              <span className="rounded bg-white/[0.05] px-1.5 py-0.5 font-mono text-[10px] text-zinc-500 tabular-nums">
                {rows.length}
              </span>
            </header>
            {shown.map((idea) => (
              <IdeaCard
                key={idea.id}
                idea={idea}
                compact={isCompleted}
                onAdvance={onAdvance}
                onEdit={onEdit}
                onDelete={onDelete}
                onOpen={onOpen}
              />
            ))}
            {rows.length === 0 && (
                  <p className="rounded-lg border border-dashed border-white/[0.07] px-3 py-6 text-center text-[11px] text-zinc-600">
                    —
                  </p>
                )}
                {isCompleted && hidden > 0 && !expandedCompleted && (
                  <button
                    type="button"
                    onClick={() => setExpandedCompleted(true)}
                    className="rounded-lg border border-white/[0.07] bg-white/[0.02] px-2 py-1.5 text-[11px] font-medium text-zinc-400 transition hover:border-amber-500/30 hover:text-amber-200"
                  >
                    {s.showMoreCompleted(hidden)}
                  </button>
                )}
            {isCompleted && expandedCompleted && rows.length > COMPLETED_PAGE && (
              <button
                type="button"
                onClick={() => setExpandedCompleted(false)}
                className="rounded-lg border border-white/[0.07] bg-white/[0.02] px-2 py-1.5 text-[11px] font-medium text-zinc-400 transition hover:text-zinc-200"
              >
                {s.showLess}
              </button>
            )}
          </section>
        );
      })}
    </div>
  );
}
