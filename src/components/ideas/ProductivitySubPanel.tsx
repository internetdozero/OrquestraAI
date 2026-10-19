import { useEffect, useMemo, useState } from "react";
import { ChevronDown, Flame, Folder, Gauge, Timer } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { devMockMode } from "../../lib/fixtures";
import { getProductivityMetrics, type ProductivityMetrics } from "../../lib/ideas";
import { cn } from "../../lib/utils";
import { CustomDropdown, type DropdownOption } from "../CustomDropdown";
import { ActivityHeatmap } from "./ActivityHeatmap";

interface ProductivitySubPanelProps {
  projects: string[];
}

function formatCycle(hours: number | null, daysSuffix: string, hoursSuffix: string): string {
  if (hours == null) return "—";
  if (hours < 48) return `${Math.round(hours)}${hoursSuffix}`;
  return `${(hours / 24).toFixed(1)}${daysSuffix}`;
}

/**
 * Velocity accordion below the ideas header: a compact streak/velocity pill
 * when collapsed, three metric cards plus the contribution heatmap when
 * open. Fetches nothing outside Tauri or browser mock mode.
 */
export function ProductivitySubPanel({ projects }: ProductivitySubPanelProps) {
  const { t } = useI18n();
  const s = t.ideas.productivity;
  const [open, setOpen] = useState(false);
  const [project, setProject] = useState("all");
  const [metrics, setMetrics] = useState<ProductivityMetrics | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || (!devMockMode() && !("__TAURI_INTERNALS__" in window))) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    getProductivityMetrics(project === "all" ? null : project, 16)
      .then((next) => {
        if (!cancelled) setMetrics(next);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(cause instanceof Error ? cause.message : t.common.unexpected);
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, project, t.common.unexpected]);

  const velocity = metrics?.weeklyVelocity;
  const collapsedSummary = metrics
    ? s.pillSummary(metrics.currentStreak, velocity?.thisWeek ?? 0)
    : s.pillIdle;

  const projectOptions = useMemo<DropdownOption<string>[]>(() => [
    { id: "all", label: s.allProjects },
    ...projects.map((name) => ({ id: name, label: name })),
  ], [projects, s.allProjects]);

  return (
    <section
      aria-label={s.title}
      className="overflow-hidden rounded-xl border border-white/[0.06] bg-black/20"
    >
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        className="flex w-full items-center gap-2 px-3.5 py-2 text-left transition hover:bg-white/[0.03]"
      >
        <Flame className="size-3.5 shrink-0 text-amber-300/90" aria-hidden="true" />
        <span className="min-w-0 flex-1 truncate text-[12px] font-medium text-zinc-200">
          {open ? s.title : collapsedSummary}
        </span>
        <ChevronDown
          className={cn("size-3.5 shrink-0 text-zinc-500 transition-transform", open && "rotate-180")}
          aria-hidden="true"
        />
      </button>

      {open && (
        <div className="space-y-3 border-t border-white/[0.05] px-3.5 py-3">
          <div className="flex flex-wrap items-center gap-2">
            <CustomDropdown
              value={project}
              options={projectOptions}
              onChange={(id) => setProject(id)}
              ariaLabel={s.projectFilter}
              title={s.projectFilter}
              icon={Folder}
            />
          </div>

          {error ? (
            <p role="alert" className="text-[11.5px] text-red-300">
              {error}
            </p>
          ) : (
            <>
              <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
                <div className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2">
                  <p className="flex items-center gap-1.5 text-[10.5px] font-medium tracking-wide text-zinc-500 uppercase">
                    <Flame className="size-3 text-amber-300/80" aria-hidden="true" />
                    {s.streak}
                  </p>
                  <p className="mt-0.5 font-mono text-[15px] text-zinc-100 tabular-nums">
                    {loading || !metrics ? "…" : s.streakValue(metrics.currentStreak)}
                  </p>
                </div>
                <div className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2">
                  <p className="flex items-center gap-1.5 text-[10.5px] font-medium tracking-wide text-zinc-500 uppercase">
                    <Gauge className="size-3 text-emerald-300/80" aria-hidden="true" />
                    {s.velocity}
                  </p>
                  <p className="mt-0.5 font-mono text-[15px] text-zinc-100 tabular-nums">
                    {loading || !metrics || !velocity
                      ? "…"
                      : s.velocityValue(velocity.thisWeek, velocity.lastWeek)}
                  </p>
                </div>
                <div className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2">
                  <p className="flex items-center gap-1.5 text-[10.5px] font-medium tracking-wide text-zinc-500 uppercase">
                    <Timer className="size-3 text-cyan-300/80" aria-hidden="true" />
                    {s.cycle}
                  </p>
                  <p className="mt-0.5 font-mono text-[15px] text-zinc-100 tabular-nums">
                    {loading || !metrics
                      ? "…"
                      : formatCycle(metrics.avgCycleHours, s.daysSuffix, s.hoursSuffix)}
                  </p>
                </div>
              </div>

              {metrics && <ActivityHeatmap days={metrics.dailyActivity} />}
            </>
          )}
        </div>
      )}
    </section>
  );
}
