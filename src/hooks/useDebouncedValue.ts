import { useEffect, useState } from "react";

/**
 * Debounced copy of `value`: updates `delayMs` after the last change, so
 * fast-typing filters (grids, catalogs) re-evaluate once per pause instead
 * of once per keystroke. The input itself stays controlled by the raw value
 * and never lags; only the expensive consumer reads the debounced one.
 */
export function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delayMs);
    return () => window.clearTimeout(timer);
  }, [value, delayMs]);
  return debounced;
}
