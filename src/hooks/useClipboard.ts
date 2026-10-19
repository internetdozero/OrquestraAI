import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Standardized clipboard write with in-place visual feedback. `copy` resolves
 * `true` on success and flips `copied` for `timeout` ms (1.5s by default,
 * matching the card and turn checkmark convention); the timer is cleared on
 * unmount so a late fire never touches a dead component.
 */
export function useClipboard(timeout = 1500) {
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current) window.clearTimeout(timerRef.current);
    };
  }, []);

  const copy = useCallback(
    async (text: string): Promise<boolean> => {
      try {
        await navigator.clipboard.writeText(text);
        setCopied(true);
        if (timerRef.current) window.clearTimeout(timerRef.current);
        timerRef.current = window.setTimeout(() => setCopied(false), timeout);
        return true;
      } catch {
        return false;
      }
    },
    [timeout],
  );

  return { copied, copy };
}
