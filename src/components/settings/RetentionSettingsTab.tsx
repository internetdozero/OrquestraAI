import { useEffect, useState } from "react";
import { ShieldCheck } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { getRetentionPolicy, saveRetentionPolicy, type RetentionPolicy } from "../../lib/sessions";
import { cn, confirmOnEnter } from "../../lib/utils";

function toMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

/** Session auto-purge controls backed by the real retention policy IPC. */
export function RetentionSettingsTab({
  isTauri,
  onToast,
  onError,
  onOpenSessions,
}: {
  isTauri: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
  onOpenSessions: () => void;
}) {
  const { t } = useI18n();
  const s = t.settings.retention;
  const [policy, setPolicy] = useState<RetentionPolicy | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!isTauri) return;
    void getRetentionPolicy()
      .then(setPolicy)
      .catch(() => undefined);
  }, [isTauri]);

  const patch = (next: Partial<RetentionPolicy>) => {
    setPolicy((prev) =>
      prev
        ? { ...prev, ...next }
        : {
            defaultTtlDays: 15,
            harnessOverrides: {},
            autoPurgeEnabled: false,
            autoPurgeIntervalHours: 24,
            lastRun: null,
            ...next,
          },
    );
  };

  const save = async () => {
    if (!isTauri || !policy || saving) return;
    setSaving(true);
    try {
      await saveRetentionPolicy(policy);
      onToast(t.sessions.saved);
    } catch (cause) {
      onError(t.sessions.saveFailed(toMessage(cause)));
    } finally {
      setSaving(false);
    }
  };

  // Policy changes apply through an explicit save so slider drags never
  // fire a burst of IPC writes.
  useEffect(() => undefined, []);

  return (
    <div className="space-y-4" onKeyDown={(event) => confirmOnEnter(event, () => void save())}>
      <div className="flex items-center justify-between gap-3 rounded-lg border border-white/[0.07] bg-white/[0.02] px-3 py-2.5">
        <div>
          <p className="flex items-center gap-1.5 text-[12.5px] text-zinc-100">
            <ShieldCheck className="size-3.5 text-emerald-400" />
            {s.enableTitle}
          </p>
          <p className="mt-0.5 text-[11px] text-zinc-500">{s.enableHint}</p>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={policy?.autoPurgeEnabled ?? false}
          aria-label={s.enableTitle}
          disabled={!isTauri}
          onClick={() => patch({ autoPurgeEnabled: !(policy?.autoPurgeEnabled ?? false) })}
          className={cn(
            "relative h-5.5 w-10 shrink-0 rounded-full transition-[background-color] disabled:opacity-50",
            policy?.autoPurgeEnabled ? "accent-dot" : "bg-zinc-700",
          )}
        >
          <span
            className={cn(
              "absolute top-0.5 size-4.5 rounded-full bg-white transition-[left]",
              policy?.autoPurgeEnabled ? "left-5" : "left-0.5",
            )}
          />
        </button>
      </div>

      <div>
        <div className="flex items-center justify-between">
          <label htmlFor="retention-ttl" className="text-[12px] text-zinc-300">
            {s.ttlTitle}
          </label>
          <span className="font-mono text-[12px] text-emerald-300">
            {policy?.defaultTtlDays ?? 15}d
          </span>
        </div>
        <input
          id="retention-ttl"
          type="range"
          min={7}
          max={60}
          value={policy?.defaultTtlDays ?? 15}
          disabled={!isTauri}
          onChange={(e) => patch({ defaultTtlDays: Number(e.target.value) })}
          className="mt-2 w-full accent-emerald-500 disabled:opacity-50"
        />
      </div>

      <div>
        <div className="flex items-center justify-between">
          <label htmlFor="retention-interval" className="text-[12px] text-zinc-300">
            {s.intervalTitle}
          </label>
          <span className="font-mono text-[12px] text-emerald-300">
            {policy?.autoPurgeIntervalHours ?? 24}h
          </span>
        </div>
        <input
          id="retention-interval"
          type="range"
          min={1}
          max={168}
          value={policy?.autoPurgeIntervalHours ?? 24}
          disabled={!isTauri}
          onChange={(e) => patch({ autoPurgeIntervalHours: Number(e.target.value) })}
          className="mt-2 w-full accent-emerald-500 disabled:opacity-50"
        />
      </div>

      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => void save()}
          disabled={!isTauri || !policy || saving}
          className="rounded-lg bg-emerald-600 px-3.5 py-1.5 text-[12px] font-semibold text-white transition hover:bg-emerald-500 disabled:opacity-50"
        >
          {t.sessions.save}
        </button>
        <button
          type="button"
          onClick={onOpenSessions}
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3.5 py-1.5 text-[12px] font-medium text-zinc-200 transition hover:border-white/20"
        >
          {s.openSessions}
        </button>
      </div>
    </div>
  );
}
