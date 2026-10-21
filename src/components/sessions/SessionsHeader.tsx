import { Database, Loader2, Pin, ShieldCheck, Trash2 } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { formatBytes } from "../../lib/sessionFormat";
import { actionButtonClass, StatTile } from "../ui";
import { cn } from "../../lib/utils";

interface SessionsHeaderProps {
  total: number;
  bytes: number;
  pinned: number;
  autoEnabled: boolean;
  intervalHours: number;
  purging: boolean;
  /** Cold start: show the scanning state instead of `0 B / 0 conversations`. */
  loading: boolean;
  onQuickPurge: (days: number | null) => void;
  onOpenRetention: () => void;
}

/** Metrics row + 1-click dry-run-backed purge shortcuts. */
export function SessionsHeader({
  total,
  bytes,
  pinned,
  autoEnabled,
  intervalHours,
  purging,
  loading,
  onQuickPurge,
  onOpenRetention,
}: SessionsHeaderProps) {
  const { t } = useI18n();
  const s = t.sessions;

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        <StatTile
          label={s.diskUsage}
          value={
            loading ? (
              <span className="flex items-center gap-2 text-[15px] font-medium text-zinc-300">
                <Loader2 className="size-4 animate-spin text-emerald-400" />
                {s.calculatingSpace}
              </span>
            ) : (
              formatBytes(bytes)
            )
          }
          hint={loading ? s.scanningDisk : `${total} · ${s.totalSessionsHint}`}
          icon={Database}
          accent
        />
        <StatTile label={s.pinned} value={`${pinned}`} hint={s.pinnedHint} icon={Pin} />
        <StatTile
          label={s.autoPurgeHint}
          value={autoEnabled ? s.autoPurgeActive(intervalHours) : s.autoPurgeDisabled}
          hint={s.retentionBody}
          icon={ShieldCheck}
        />
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {[
          { label: s.purge7, days: 7 },
          { label: s.purge15, days: 15 },
          { label: s.purge30, days: 30 },
        ].map((action) => (
          <button
            key={action.days}
            type="button"
            disabled={purging || loading || total === 0}
            onClick={() => onQuickPurge(action.days)}
            className={cn(actionButtonClass, "flex items-center gap-1.5")}
          >
            <Trash2 className="size-3.5 text-amber-400" />
            {action.label}
          </button>
        ))}
        <button
          type="button"
          disabled={purging || loading || total === 0}
          onClick={() => onQuickPurge(null)}
          className={cn(
            actionButtonClass,
            "flex items-center gap-1.5 border-rose-500/30 text-rose-200 hover:border-rose-500/50",
          )}
        >
          <Trash2 className="size-3.5" />
          {s.purgeAll}
        </button>
        <button
          type="button"
          onClick={onOpenRetention}
          className={cn(actionButtonClass, "ml-auto flex items-center gap-1.5")}
        >
          <ShieldCheck className="size-3.5 text-emerald-400" />
          {s.configureRetention}
        </button>
      </div>
    </div>
  );
}
