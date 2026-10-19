import { Archive, FolderGit2, Gauge, Search } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { CustomDropdown } from "../CustomDropdown";
import { cn } from "../../lib/utils";

interface IdeasToolbarProps {
  search: string;
  onSearchChange: (value: string) => void;
  projects: string[];
  selectedProject: string;
  onProjectChange: (value: string) => void;
  selectedCategory: string;
  onCategoryChange: (value: string) => void;
  selectedPriority: string;
  onPriorityChange: (value: string) => void;
  /** Archived ideas live in a dedicated modal; the badge counts them. */
  archivedCount?: number;
  onOpenArchived: () => void;
}

const CATEGORY_IDS = ["feature", "architecture", "bug", "ux", "idea"] as const;
const PRIORITY_IDS = ["low", "medium", "high", "critical"] as const;

/**
 * One filter row: search on the left, category chips in the middle, project
 * and priority selectors pinned right. Wraps gracefully on narrow screens.
 */
export function IdeasToolbar({
  search,
  onSearchChange,
  projects,
  selectedProject,
  onProjectChange,
  selectedCategory,
  onCategoryChange,
  selectedPriority,
  onPriorityChange,
  archivedCount = 0,
  onOpenArchived,
  }: IdeasToolbarProps) {
  const { t } = useI18n();
  const s = t.ideas;

  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
      <label className="relative min-w-44 flex-1 basis-52">
        <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-zinc-500" />
        <input
          value={search}
          onChange={(event) => onSearchChange(event.target.value)}
          placeholder={s.searchPlaceholder}
          aria-label={s.searchAria}
          spellCheck={false}
          className="h-8.5 w-full rounded-lg border border-white/10 bg-white/[0.03] pr-2.5 pl-8 text-[12px] text-zinc-200 placeholder:text-zinc-600 outline-none transition focus:border-amber-500/40 focus:ring-2 focus:ring-amber-500/10"
        />
      </label>

      <div
        className="flex min-w-0 flex-1 basis-80 flex-wrap items-center justify-center gap-1.5"
        role="group"
        aria-label={s.categoryAria}
      >
        {[{ id: "all", label: s.all }].concat(
          CATEGORY_IDS.map((id) => ({ id, label: s.categories[id] })),
        ).map((option) => (
          <button
            key={option.id}
            type="button"
            onClick={() => onCategoryChange(option.id)}
            aria-pressed={selectedCategory === option.id}
            className={cn(
              "rounded-full border px-2.5 py-1 text-[11px] font-medium transition",
              selectedCategory === option.id
                ? "border-amber-500/40 bg-amber-500/15 text-amber-200"
                : "border-white/10 bg-white/[0.02] text-zinc-400 hover:border-white/20 hover:text-zinc-200",
            )}
          >
            {option.label}
          </button>
        ))}
      </div>

      <div className="ml-auto flex shrink-0 items-center gap-2">
        <button
          type="button"
          onClick={onOpenArchived}
          title={s.archivedModalTitle}
          aria-label={s.archivedModalTitle}
          className="relative grid size-8.5 place-items-center rounded-lg border border-white/10 bg-white/[0.03] text-zinc-400 transition hover:border-white/20 hover:text-zinc-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/40"
        >
          <Archive className="size-4" />
          {archivedCount > 0 && (
            <span className="absolute -top-1 -right-1 flex size-4 items-center justify-center rounded-full border border-amber-500/40 bg-amber-500/20 font-mono text-[9px] font-semibold text-amber-300">
              {archivedCount}
            </span>
          )}
        </button>
        <CustomDropdown
          value={selectedProject}
          ariaLabel={s.projectAria}
          icon={FolderGit2}
          options={[{ id: "all", label: s.allProjects }].concat(
            projects.map((project) => ({ id: project, label: project })),
          )}
          onChange={onProjectChange}
        />
        <CustomDropdown
          value={selectedPriority}
          ariaLabel={s.priorityAria}
          icon={Gauge}
          options={[{ id: "all", label: s.priorityAll }].concat(
            PRIORITY_IDS.map((id) => ({ id, label: s.priorities[id] })),
          )}
          onChange={onPriorityChange}
        />
      </div>
    </div>
  );
}
