import { useId, useState } from "react";
import type { DayActivity } from "../../lib/ideas";
import { cn } from "../../lib/utils";

/** Cell fills per intensity level: calm emerald ramp, amber for peak days. */
const LEVEL_STYLE = [
  "bg-white/[0.04] hover:border-white/20",
  "bg-emerald-950 hover:border-emerald-700/60",
  "bg-emerald-800/70 hover:border-emerald-500/60",
  "bg-emerald-500/80 hover:border-emerald-300/60",
  "bg-amber-400 hover:border-amber-200",
];

function cellLabel(day: DayActivity): string {
  const date = new Date(`${day.date}T12:00:00`);
  const pretty = Number.isNaN(date.getTime()) ? day.date : date.toLocaleDateString();
  const ideas =
    day.ideasCompleted === 1 ? "1 idea completed" : `${day.ideasCompleted} ideas completed`;
  return `${pretty}: ${ideas}`;
}

/**
 * GitHub-style contribution grid: columns are weeks, rows are Sunday-Saturday.
 * Pure CSS grid (no chart library); the hovered cell reports its breakdown
 * through a floating tooltip instead of a bare `title`.
 */
export function ActivityHeatmap({ days }: { days: DayActivity[] }) {
  const tooltipId = useId();
  const [active, setActive] = useState<DayActivity | null>(null);
  if (days.length === 0) return null;

  const firstWeekday = new Date(`${days[0].date}T12:00:00`).getDay();
  const leading = Number.isNaN(firstWeekday) ? 0 : firstWeekday;

  return (
    <div className="overflow-x-auto pb-1">
      <div
        role="img"
        aria-label="Daily activity heatmap"
        aria-describedby={active ? tooltipId : undefined}
        className="grid min-w-max auto-cols-[11px] grid-flow-col gap-[3px]"
        style={{ gridTemplateRows: "repeat(7, 11px)" }}
        onMouseLeave={() => setActive(null)}
      >
        {Array.from({ length: leading }, (_, index) => (
          <span key={`pad-${index}`} className="size-[11px]" aria-hidden="true" />
        ))}
        {days.map((day) => (
          <button
            key={day.date}
            type="button"
            onMouseEnter={() => setActive(day)}
            onFocus={() => setActive(day)}
            onClick={() => setActive(day)}
            aria-label={cellLabel(day)}
            className={cn(
              "size-[11px] rounded-[3px] border border-transparent transition-colors",
              LEVEL_STYLE[Math.min(day.intensity, 4)],
            )}
          />
        ))}
      </div>
      <p
        id={tooltipId}
        role="status"
        className={cn(
          "mt-2 font-mono text-[10.5px] text-zinc-400 tabular-nums",
          !active && "invisible",
        )}
      >
        {active ? cellLabel(active) : "·"}
      </p>
    </div>
  );
}
