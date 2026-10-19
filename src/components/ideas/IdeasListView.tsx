import { useState } from "react";
import { ArrowDown, ArrowUp, ArrowUpDown, Check, ChevronDown, ChevronRight, Copy, Inbox, Pencil, Trash2 } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import type { IdeaItem } from "../../lib/ideas";
import { copyIdeaForAi } from "../../lib/ideas";
import { cn } from "../../lib/utils";
import { CustomDropdown } from "../CustomDropdown";
import { StatePanel } from "../ui";

interface IdeasListViewProps {
  ideas: IdeaItem[];
  loading: boolean;
  onOpen: (idea: IdeaItem) => void;
  onStatusChange: (id: number, status: string) => void;
  onEdit: (idea: IdeaItem) => void;
  onDelete: (idea: IdeaItem) => void;
}

const SECTIONS = [
  { status: "open", titleKey: "colOpen" },
  { status: "in_progress", titleKey: "colInProgress" },
  { status: "completed", titleKey: "colCompleted" },
] as const;

const STATUS_IDS = ["open", "in_progress", "completed", "archived"] as const;

/** Notion-style sortable columns of the list header. */
type SortField = "title" | "project" | "category" | "priority" | "updated";

const PRIORITY_RANK: Record<string, number> = {
  low: 0,
  medium: 1,
  high: 2,
  critical: 3,
};

function compareIdeas(field: SortField, a: IdeaItem, b: IdeaItem): number {
  switch (field) {
    case "title":
      return a.title.localeCompare(b.title);
    case "project":
      return a.project.localeCompare(b.project);
    case "category":
      return a.category.localeCompare(b.category);
    case "priority":
      return (PRIORITY_RANK[a.priority] ?? -1) - (PRIORITY_RANK[b.priority] ?? -1);
    case "updated":
      return (
        Date.parse(a.updatedAt.replace(" ", "T") + "Z") -
        Date.parse(b.updatedAt.replace(" ", "T") + "Z")
      );
  }
}

/** One clickable header cell with a direction indicator. */
function SortHeader({
  label,
  field,
  sortField,
  sortAsc,
  onSort,
}: {
  label: string;
  field: SortField;
  sortField: SortField | null;
  sortAsc: boolean;
  onSort: (field: SortField) => void;
}) {
  const active = sortField === field;
  return (
    <button
      type="button"
      onClick={() => onSort(field)}
      title={label}
      aria-label={active ? `${label} (${sortAsc ? "ascending" : "descending"})` : `Sort by ${label}`}
      className="group/sort flex min-w-0 cursor-pointer items-center gap-1 truncate transition-colors hover:text-zinc-200"
    >
      <span className="truncate">{label}</span>
      {active ? (
        sortAsc ? (
          <ArrowUp className="size-3 shrink-0 text-amber-300" aria-hidden="true" />
        ) : (
          <ArrowDown className="size-3 shrink-0 text-amber-300" aria-hidden="true" />
        )
      ) : (
        <ArrowUpDown className="size-3 shrink-0 opacity-0 transition-opacity group-hover/sort:opacity-60" aria-hidden="true" />
      )}
    </button>
  );
}

/**
 * Grouped list alternative to the kanban: collapsible status sections with
 * compact rows (title, project, category, priority, updated) and an inline
 * status selector per row.
 */
export function IdeasListView({
  ideas,
  loading,
  onOpen,
  onStatusChange,
  onEdit,
  onDelete,
}: IdeasListViewProps) {
  const { t } = useI18n();
  const s = t.ideas;
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(() => new Set());
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const [sortField, setSortField] = useState<SortField | null>(null);
  const [sortAsc, setSortAsc] = useState(true);

  const sortBy = (field: SortField) => {
    if (sortField === field) {
      setSortAsc((asc) => !asc);
    } else {
      setSortField(field);
      setSortAsc(true);
    }
  };

  if (!loading && ideas.length === 0) {
    return <StatePanel icon={Inbox} title={s.empty} description={s.emptyHint} />;
  }

  const toggle = (status: string) =>
    setCollapsed((previous) => {
      const next = new Set(previous);
      if (next.has(status)) next.delete(status);
      else next.add(status);
      return next;
    });

  return (
    // One bordered table box: the header row and every data row share the
    // exact column template, padding and gap, so columns line up by
    // construction instead of by coincidence.
    <div className="overflow-hidden rounded-xl border border-white/[0.05] bg-black/20">
      {/* Interactive column labels for wide screens; rows carry the same fields inline. */}
      <div
        className="hidden grid-cols-[minmax(0,1fr)_120px_100px_90px_100px_130px_70px] items-center gap-4 border-b border-white/[0.08] px-4 pt-2.5 pb-2 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase lg:grid"
      >
        <SortHeader label={s.hTitle} field="title" sortField={sortField} sortAsc={sortAsc} onSort={sortBy} />
        <SortHeader label={s.fieldProject} field="project" sortField={sortField} sortAsc={sortAsc} onSort={sortBy} />
        <SortHeader label={s.fieldCategory} field="category" sortField={sortField} sortAsc={sortAsc} onSort={sortBy} />
        <SortHeader label={s.fieldPriority} field="priority" sortField={sortField} sortAsc={sortAsc} onSort={sortBy} />
        <SortHeader label={s.hUpdated} field="updated" sortField={sortField} sortAsc={sortAsc} onSort={sortBy} />
        <span className="truncate">{s.fieldStatus}</span>
        <span className="sr-only">actions</span>
      </div>

      {SECTIONS.map((section) => {
        const base = ideas.filter((idea) => idea.status === section.status);
        if (base.length === 0) return null;
        const rows = sortField
          ? [...base].sort((a, b) =>
              sortAsc ? compareIdeas(sortField, a, b) : -compareIdeas(sortField, a, b),
            )
          : base;
        const isCollapsed = collapsed.has(section.status);

        return (
          <section key={section.status} aria-label={s[section.titleKey]}>
            <button
              type="button"
              onClick={() => toggle(section.status)}
              aria-expanded={!isCollapsed}
              className="flex w-full items-center gap-2 px-4 py-2 text-left transition hover:bg-white/[0.03]"
            >
              {isCollapsed ? (
                <ChevronRight className="size-3.5 shrink-0 text-zinc-500" aria-hidden="true" />
              ) : (
                <ChevronDown className="size-3.5 shrink-0 text-zinc-500" aria-hidden="true" />
              )}
              <h3 className="text-[11px] font-semibold tracking-[0.1em] text-zinc-400 uppercase">
                {s[section.titleKey]}
              </h3>
              <span className="rounded bg-white/[0.05] px-1.5 py-0.5 font-mono text-[10px] text-zinc-500 tabular-nums">
                {rows.length}
              </span>
            </button>

            {!isCollapsed && (
              <ol className="divide-y divide-white/[0.04] border-t border-white/[0.05]">
                {rows.map((idea) => {
                  const updated = new Date(idea.updatedAt.replace(" ", "T") + "Z");
                  return (
                    <li
                      key={idea.id}
                      className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-4 px-4 py-2 transition hover:bg-white/[0.02] lg:grid-cols-[minmax(0,1fr)_120px_100px_90px_100px_130px_70px]"
                    >
                      <div className="min-w-0">
                        <button
                          type="button"
                          onClick={() => onOpen(idea)}
                          className="block w-full truncate text-left text-[12.5px] font-medium text-zinc-100 hover:text-amber-200"
                        >
                          {idea.title}
                        </button>
                        {/* Inline status on small screens, where the table columns collapse. */}
                        <div className="mt-1 max-w-44 lg:hidden" onClick={(event) => event.stopPropagation()}>
                          <CustomDropdown
                            value={idea.status}
                            ariaLabel={s.fieldStatus}
                            options={STATUS_IDS.map((id) => ({
                              id,
                              label: s.statusNames[id] ?? id,
                            }))}
                            onChange={(status) => {
                              if (status !== idea.status) onStatusChange(idea.id, status);
                            }}
                            className="w-full [&>button]:w-full [&>button]:justify-between"
                          />
                        </div>
                      </div>
                      <span className="hidden truncate text-left font-mono text-[11px] text-zinc-400 lg:block">
                        {idea.project}
                      </span>
                      <span className="hidden truncate text-left text-[11px] text-zinc-400 lg:block">
                        {s.categories[idea.category as keyof typeof s.categories] ?? idea.category}
                      </span>
                      <span className="hidden truncate text-left text-[11px] text-zinc-400 lg:block">
                        {s.priorities[idea.priority as keyof typeof s.priorities] ?? idea.priority}
                      </span>
                      <span className="hidden truncate text-left text-[11px] text-zinc-500 tabular-nums lg:block">
                        {!Number.isNaN(updated.getTime()) ? updated.toLocaleDateString() : "—"}
                      </span>
                      <span
                        className="hidden min-w-0 lg:block"
                        onClick={(event) => event.stopPropagation()}
                      >
                        <CustomDropdown
                          value={idea.status}
                          ariaLabel={s.fieldStatus}
                          options={STATUS_IDS.map((id) => ({
                            id,
                            label: s.statusNames[id] ?? id,
                          }))}
                          onChange={(status) => {
                            if (status !== idea.status) onStatusChange(idea.id, status);
                          }}
                          className="w-full [&>button]:w-full [&>button]:justify-between"
                        />
                      </span>
                      <span className="flex items-center justify-end gap-1.5">
                        <button
                          type="button"
                          onClick={(event) => {
                            event.stopPropagation();
                            void copyIdeaForAi(idea).then((ok) => {
                              if (!ok) return;
                              setCopiedId(idea.id);
                              window.setTimeout(() => setCopiedId(null), 1500);
                            });
                          }}
                          title={copiedId === idea.id ? s.copied : s.copyForAi}
                          aria-label={copiedId === idea.id ? s.copied : s.copyForAi}
                          className={cn(
                            "grid size-6.5 shrink-0 place-items-center rounded-md border transition",
                            copiedId === idea.id
                              ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-400"
                              : "border-white/10 text-zinc-500 hover:text-zinc-200",
                          )}
                        >
                          {copiedId === idea.id ? (
                            <Check className="size-3" />
                          ) : (
                            <Copy className="size-3" />
                          )}
                        </button>
                        <button
                          type="button"
                          onClick={() => onEdit(idea)}
                          title={s.edit}
                          aria-label={s.edit}
                          className="grid size-6.5 shrink-0 place-items-center rounded-md border border-white/10 text-zinc-500 transition hover:text-zinc-200"
                        >
                          <Pencil className="size-3" />
                        </button>
                        <button
                          type="button"
                          onClick={() => onDelete(idea)}
                          title={s.delete}
                          aria-label={s.delete}
                          className="grid size-6.5 shrink-0 place-items-center rounded-md border border-white/10 text-zinc-500 transition hover:border-rose-500/40 hover:text-rose-300"
                        >
                          <Trash2 className="size-3" />
                        </button>
                      </span>
                    </li>
                  );
                })}
              </ol>
            )}
          </section>
        );
      })}
    </div>
  );
}
