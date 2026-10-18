import { useCallback, useEffect, useMemo, useState } from "react";
import { GLOBAL_SCOPE, type HarnessSummary } from "../lib/harnesses";
import { harnessSummary } from "../lib/tauri";

/**
 * Per-harness active-skill counters for the sidebar and the vault totals.
 * `refresh()` is called after every projection change so the badges catch up.
 */
export function useHarnessSummaries(
  isTauri: boolean,
  onError: (message: string) => void,
  reloadToken = 0,
) {
  const [summaries, setSummaries] = useState<HarnessSummary[]>([]);

  const refresh = useCallback(async () => {
    if (!isTauri) return;

    try {
      setSummaries(await harnessSummary());
    } catch (cause) {
      onError(
        `Could not refresh harness counters: ${
          cause instanceof Error ? cause.message : String(cause)
        }`,
      );
    }
  }, [isTauri, onError]);

  useEffect(() => {
    void refresh();
  }, [refresh, reloadToken]);

  const counts = useMemo(
    () =>
      Object.fromEntries(summaries.map((row) => [row.id, row.active])) as Record<
        string,
        number
      >,
    [summaries],
  );

  const total =
    summaries.find((row) => row.id === GLOBAL_SCOPE.id)?.total ?? 0;

  return { summaries, counts, total, refresh };
}
