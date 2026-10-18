import { useCallback, useEffect, useState } from "react";
import { useI18n } from "./useI18n";
import { getSystemHealth, sweepSystemCaches, type SystemHealth } from "../lib/janitor";

export function useJanitor(options: {
  isTauri: boolean;
  /** Gates the sweep-completion toast; errors always surface. */
  notifyOnPurge: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
}) {
  const { isTauri, notifyOnPurge, onToast, onError } = options;
  const { t } = useI18n();
  const [health, setHealth] = useState<SystemHealth | null>(null);
  const [loading, setLoading] = useState(false);
  const [sweeping, setSweeping] = useState(false);

  const refresh = useCallback(async () => {
    if (!isTauri) return;
    setLoading(true);
    try {
      setHealth(await getSystemHealth());
    } catch (cause) {
      const detail = cause instanceof Error ? cause.message : String(cause);
      onError(t.janitor.readFailed(detail));
    } finally {
      setLoading(false);
    }
  }, [isTauri, onError, t]);

  useEffect(() => {
    if (!isTauri) return;
    void refresh();
    const timer = window.setInterval(() => void refresh(), 5_000);
    return () => window.clearInterval(timer);
  }, [isTauri, refresh]);

  const sweep = useCallback(async () => {
    if (!isTauri || sweeping) return;
    setSweeping(true);
    try {
      const report = await sweepSystemCaches();
      if (notifyOnPurge) {
        onToast(t.janitor.sweepComplete(report.removedCount, report.reclaimedMb));
      }
      await refresh();
    } catch (cause) {
      const detail = cause instanceof Error ? cause.message : String(cause);
      onError(t.janitor.sweepFailed(detail));
    } finally {
      setSweeping(false);
    }
  }, [isTauri, notifyOnPurge, onError, onToast, refresh, sweeping, t]);

  return { health, loading, sweeping, refresh, sweep };
}
