import { Activity } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { cn } from "../../lib/utils";

interface JanitorSettingsTabProps {
  ramAlertThreshold: number;
  tmpWatchEnabled: boolean;
  onRamThreshold: (value: number) => void;
  onToggleTmpWatch: () => void;
}

/** RAM alert slider + /tmp watcher toggle. */
export function JanitorSettingsTab({
  ramAlertThreshold,
  tmpWatchEnabled,
  onRamThreshold,
  onToggleTmpWatch,
}: JanitorSettingsTabProps) {
  const { t } = useI18n();
  const j = t.settings.janitor;

  return (
    <div className="space-y-4">
      <div>
        <div className="flex items-center justify-between">
          <label
            htmlFor="janitor-ram"
            className="flex items-center gap-1.5 text-[12px] text-zinc-200"
          >
            <Activity className="size-3.5 text-amber-400" />
            {j.ramTitle}
          </label>
          <span
            className={cn(
              "font-mono text-[12px]",
              ramAlertThreshold >= 90 ? "text-rose-300" : "text-amber-300",
            )}
          >
            {ramAlertThreshold}%
          </span>
        </div>
        <input
          id="janitor-ram"
          type="range"
          min={70}
          max={95}
          value={ramAlertThreshold}
          onChange={(e) => onRamThreshold(Number(e.target.value))}
          className="mt-2 w-full accent-amber-500"
        />
        <p className="mt-1 text-[11px] text-zinc-500">{j.ramHint}</p>
      </div>

      <div className="flex items-center justify-between gap-3 rounded-lg border border-white/[0.07] bg-white/[0.02] px-3 py-2.5">
        <div>
          <p className="flex items-center gap-1.5 text-[12.5px] text-zinc-100">
            {j.tmpTitle}
            {/* No background /tmp watcher exists yet: the toggle is kept
                visible but honestly marked until the daemon lands. */}
            <span className="rounded bg-white/[0.06] px-1.5 py-px text-[10px] font-medium text-zinc-500">
              {j.soonBadge}
            </span>
          </p>
          <p className="mt-0.5 text-[11px] text-zinc-500">{j.tmpHint}</p>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={tmpWatchEnabled}
          aria-label={j.tmpTitle}
          onClick={onToggleTmpWatch}
          className={cn(
            "relative h-5.5 w-10 shrink-0 rounded-full transition-[background-color]",
            tmpWatchEnabled ? "accent-dot" : "bg-zinc-700",
          )}
        >
          <span
            className={cn(
              "absolute top-0.5 size-4.5 rounded-full bg-white transition-[left]",
              tmpWatchEnabled ? "left-5" : "left-0.5",
            )}
          />
        </button>
      </div>
    </div>
  );
}
