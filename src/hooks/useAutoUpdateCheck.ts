import { useEffect, useState } from "react";
import { checkLatestRelease, type ReleaseCheckResult } from "../lib/updater";

interface AutoUpdateCheckOptions {
  enabled: boolean;
  currentVersion: string | null;
  isTauri: boolean;
  onUpdateFound?: (result: ReleaseCheckResult) => void;
  delayMs?: number;
  intervalMs?: number;
}

const STARTUP_DELAY_MS = 4000;
const BACKGROUND_INTERVAL_MS = 4 * 60 * 60 * 1000;

/** Background release poll: delayed first check, then every 4h. */
export function useAutoUpdateCheck({
  enabled,
  currentVersion,
  isTauri,
  onUpdateFound,
  delayMs = STARTUP_DELAY_MS,
  intervalMs = BACKGROUND_INTERVAL_MS,
}: AutoUpdateCheckOptions) {
  const [availableRelease, setAvailableRelease] = useState<ReleaseCheckResult | null>(null);

  useEffect(() => {
    if (!enabled || !isTauri || !currentVersion) return;
    let cancelled = false;
    const runCheck = async () => {
      try {
        const result = await checkLatestRelease(currentVersion);
        if (!cancelled && !result.isLatest) {
          setAvailableRelease(result);
          onUpdateFound?.(result);
        }
      } catch {
        /* Network failures resolve as up-to-date; ignore. */
      }
    };
    const startupTimer = window.setTimeout(runCheck, delayMs);
    const interval = window.setInterval(runCheck, intervalMs);
    return () => {
      cancelled = true;
      window.clearTimeout(startupTimer);
      window.clearInterval(interval);
    };
  }, [enabled, currentVersion, isTauri, onUpdateFound, delayMs, intervalMs]);

  return { availableRelease, hasUpdate: availableRelease !== null };
}
