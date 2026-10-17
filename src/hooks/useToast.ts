import { useCallback, useEffect, useRef, useState } from "react";

/** Transient status message, auto-dismissed after a few seconds. */
export function useToast(timeoutMs = 3200) {
  const [toast, setToast] = useState<string | null>(null);
  const timer = useRef<number | null>(null);

  const showToast = useCallback(
    (message: string) => {
      setToast(message);
      if (timer.current !== null) window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => {
        setToast(null);
        timer.current = null;
      }, timeoutMs);
    },
    [timeoutMs],
  );

  useEffect(
    () => () => {
      if (timer.current !== null) window.clearTimeout(timer.current);
    },
    [],
  );

  return { toast, showToast };
}
