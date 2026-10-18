import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { cn } from "../lib/utils";

export interface ScopeTab {
  id: string;
  label: string;
  /** Tooltip: paths, counters, or why the scope is dimmed. */
  title?: string;
  icon?: LucideIcon;
  /** Small trailing marker, for example an active-skill count. */
  badge?: ReactNode;
  /** Known to the catalog but missing on this machine. */
  dimmed?: boolean;
}

/**
 * Inner scope switcher shared by the skills grid and the MCP registry.
 *
 * A scope is a *toggle target*, not a route: picking one re-evaluates what the
 * current view shows without leaving the view, so the shell never remounts and
 * the grid keeps its scroll position.
 */
export function ScopeTabs({
  label,
  scope,
  tabs,
  onSelect,
}: {
  label: string;
  scope: string;
  tabs: readonly ScopeTab[];
  onSelect: (scope: string) => void;
}) {
  return (
    <div className="mb-4 flex flex-wrap items-center gap-1.5 rounded-lg border border-white/[0.06] bg-white/[0.02] p-1.5">
      <span className="px-1.5 text-[9.5px] font-medium tracking-[0.12em] text-zinc-600 uppercase">
        {label}
      </span>

      {tabs.map((tab) => {
        const Icon = tab.icon;
        const active = scope === tab.id;

        return (
          <button
            key={tab.id}
            type="button"
            onClick={() => onSelect(tab.id)}
            aria-pressed={active}
            title={tab.title}
            className={cn(
              "flex items-center gap-1.5 rounded px-2 py-1 text-[11px] font-medium transition",
              active
                ? "accent-bg-soft accent-text"
                : tab.dimmed
                  ? "text-zinc-700 hover:text-zinc-500"
                  : "text-zinc-500 hover:bg-white/[0.04] hover:text-zinc-300",
            )}
          >
            {Icon && (
              <Icon
                className={cn(
                  "size-3 shrink-0",
                  active ? "accent-text" : "text-zinc-600",
                )}
              />
            )}
            <span className="whitespace-nowrap">{tab.label}</span>
            {tab.badge}
          </button>
        );
      })}
    </div>
  );
}
