import { useCallback, useRef } from "react";

/** Copies a secret and clears the system clipboard after the requested delay. */
export function useTimedClipboard(clearAfterMs = 20_000) {
  const timer = useRef<number | null>(null);

  const copy = useCallback(
    async (text: string): Promise<boolean> => {
      try {
        await navigator.clipboard.writeText(text);
        if (timer.current !== null) window.clearTimeout(timer.current);
        timer.current = window.setTimeout(() => {
          void navigator.clipboard.writeText("").catch(() => undefined);
          timer.current = null;
        }, clearAfterMs);
        return true;
      } catch {
        return false;
      }
    },
    [clearAfterMs],
  );

  return { copy };
}
