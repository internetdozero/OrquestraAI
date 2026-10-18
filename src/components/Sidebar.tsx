import { Lock, Settings } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import { HARNESSES } from "../lib/harnesses";
import { VIEWS, type ActiveView, type ViewDefinition } from "../lib/views";
import { cn } from "../lib/utils";

interface SidebarProps {
  view: ActiveView;
  onSelect: (view: ActiveView) => void;
  /** Active skills in whichever scope the skills view is showing. */
  skillStats: { active: number; total: number };
  mcpCount: number;
  onOpenSettings: () => void;
}

function CountBadge({ value, active }: { value: number; active: boolean }) {
  return (
    <span
      className={cn(
        "min-w-6 rounded px-1.5 py-0.5 text-center font-mono text-[10px] tabular-nums",
        active
          ? "accent-bg-soft accent-text"
          : value > 0
            ? "bg-white/[0.05] text-zinc-400"
            : "bg-white/[0.03] text-zinc-600",
      )}
    >
      {value}
    </span>
  );
}

function ViewButton({
  view,
  label,
  hint,
  active,
  count,
  onSelect,
}: {
  view: ViewDefinition;
  label: string;
  hint: string;
  active: boolean;
  count: number | null;
  onSelect: (view: ActiveView) => void;
}) {
  const Icon = view.icon;

  return (
    <button
      type="button"
      onClick={() => onSelect(view.id)}
      title={hint}
      aria-current={active ? "page" : undefined}
      className={cn(
        "group relative flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[12.5px] transition-colors",
        active
          ? "accent-bg-soft accent-text"
          : "text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200",
      )}
    >
      {active && (
        <span className="accent-dot absolute top-1/2 left-0 h-4 w-0.5 -translate-y-1/2 rounded-r-full" />
      )}
      <Icon
        className={cn(
          "size-4 shrink-0 transition-colors",
          active
            ? "accent-text"
            : "text-zinc-500 group-hover:text-zinc-300",
        )}
      />
      <span className="min-w-0 flex-1 truncate font-medium">{label}</span>
      {count === null ? (
        active && <span className="accent-dot size-1.5 rounded-full" />
      ) : (
        <CountBadge value={count} active={active} />
      )}
    </button>
  );
}

/**
 * Platform navigation. Harness scopes live *inside* the skills and MCP views as
 * inner scope tabs, so this list stays at the three things the app manages.
 */
export function Sidebar({ view, onSelect, skillStats, mcpCount, onOpenSettings }: SidebarProps) {
  const { t } = useI18n();
  const counts: Record<ActiveView, number | null> = {
    skills: skillStats.active,
    mcps: mcpCount,
    agents: null,
    sessions: null,
    ideas: null,
  };

  return (
    <aside className="relative z-10 flex w-60 shrink-0 flex-col border-r border-white/[0.06] bg-[#0b0b0e]/70">
      <div className="flex-1 overflow-y-auto px-2 py-4">
        <div className="px-2 pb-2">
          <p className="text-[10px] font-medium tracking-[0.14em] text-zinc-600 uppercase">
            {t.sidebar.workspace}
          </p>
          <p className="mt-0.5 font-mono text-[10px] text-zinc-600">
            {t.sidebar.counts(skillStats.total, HARNESSES.length)}
          </p>
        </div>

        <div className="space-y-0.5">
          {VIEWS.map((definition) => {
            const copy = t.sidebar.views[definition.id];
            return (
              <ViewButton
                key={definition.id}
                view={definition}
                label={copy.label}
                hint={copy.hint}
                active={view === definition.id}
                count={counts[definition.id]}
                onSelect={onSelect}
              />
            );
          })}
        </div>

        <div className="mt-4 rounded-lg border border-white/[0.05] bg-white/[0.02] p-2.5">
          <p className="flex items-center gap-1.5 text-[9.5px] font-medium tracking-[0.12em] text-zinc-500 uppercase">
            <Lock className="size-2.5" />
            {t.sidebar.inheritance.title}
          </p>
          <p className="mt-1 text-[10.5px] leading-relaxed text-zinc-600">
            {t.sidebar.inheritance.body}
          </p>
        </div>
      </div>
      <div className="flex shrink-0 items-center justify-between border-t border-white/[0.06] px-3 py-2.5">
        <button
          type="button"
          onClick={onOpenSettings}
          title={t.settings.sidebarHint}
          aria-label={t.settings.sidebarAction}
          className="flex items-center gap-2 rounded-lg px-2 py-1.5 text-[12px] font-medium text-zinc-400 transition hover:bg-white/[0.04] hover:text-zinc-100 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none"
        >
          <Settings className="size-4" />
          {t.settings.sidebarAction}
        </button>
        <span className="font-mono text-[10px] text-zinc-600">v0.1.0</span>
      </div>
    </aside>
  );
}
