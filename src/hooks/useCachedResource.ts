import { useCallback, useEffect, useRef, useState } from "react";
import {
  isCacheFresh,
  loadCacheValue,
  readCacheValue,
  reloadCacheValue,
  writeCacheValue,
} from "../lib/dataCache";
import { perfViewPainted } from "../lib/perf";

export interface CachedResourceOptions<T> {
  /** Cache identity; switching keys adopts the other entry during render. */
  key: string;
  /** How long a stored value is served without a background read. */
  ttlMs: number;
  /** Skip every read when false (browser preview, fixtures off). */
  enabled: boolean;
  /** Painted until the first read lands. */
  fallback: T;
  /** One core read. Stable identity is not required: it is kept in a ref. */
  load: () => Promise<T>;
  /** View id to time: reports cache-vs-load on the switch that opened it. */
  perfView?: string;
}

/**
 * Cache-first view of one IPC resource.
 *
 * Behaviour, in the order it matters to the operator:
 *
 * 1. A cached value paints on the very first render — remounting a view (which
 *    is what every tab switch does here) never shows an empty state again.
 * 2. A value younger than `ttlMs` is served as-is: no spinner, no re-read.
 * 3. A stale value is served immediately and revalidated in the background;
 *    `revalidating` reports that, and the list is replaced in place.
 * 4. Only a key with nothing cached sets `loading`, which is the one case that
 *    legitimately blocks with a skeleton.
 *
 * `refresh()` forces a read (explicit refresh, post-mutation), `ensure()`
 * revalidates only when the TTL expired (polling, window focus) and `mutate()`
 * applies an optimistic value that in-flight replies cannot overwrite.
 */
export function useCachedResource<T>(options: CachedResourceOptions<T>) {
  const { key, ttlMs, enabled, fallback, perfView } = options;

  // The loader closes over props (locale, callbacks); a ref keeps `run` stable
  // so an unrelated re-render never restarts the read.
  const loadRef = useRef(options.load);
  loadRef.current = options.load;

  const [data, setData] = useState<T>(() => readCacheValue<T>(key) ?? fallback);
  const [loading, setLoading] = useState(() => enabled && readCacheValue<T>(key) === null);
  const [revalidating, setRevalidating] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const requestRef = useRef(0);

  // Adopt the new key's entry during render: a scope/key switch must not paint
  // the previous resource for a frame.
  const [shownKey, setShownKey] = useState(key);
  if (shownKey !== key) {
    setShownKey(key);
    setData(readCacheValue<T>(key) ?? fallback);
    setLoading(enabled && readCacheValue<T>(key) === null);
    setError(null);
  }

  const run = useCallback(
    async (mode: "ensure" | "force") => {
      if (!enabled) {
        setLoading(false);
        return;
      }

      const request = requestRef.current + 1;
      requestRef.current = request;
      const request$ = { key, ttlMs, load: () => loadRef.current() };
      const cached = readCacheValue<T>(key);

      // Nothing to do: the entry is fresh and this is not an explicit refresh.
      if (mode === "ensure" && isCacheFresh(key, ttlMs)) {
        setLoading(false);
        return;
      }

      if (cached === null) setLoading(true);
      else setRevalidating(true);

      try {
        const next =
          mode === "force" ? await reloadCacheValue(request$) : await loadCacheValue(request$);
        if (request !== requestRef.current) return;
        setData(next);
        setError(null);
        // Only a cold read is the switch's first data; a repaint from cache was
        // already reported when this hook mounted.
        if (perfView && cached === null) perfViewPainted(perfView, "load");
      } catch (cause) {
        if (request === requestRef.current) setError(cause);
      } finally {
        if (request === requestRef.current) {
          setLoading(false);
          setRevalidating(false);
        }
      }
    },
    [enabled, key, ttlMs, perfView],
  );

  // Mount (and key change): serve the cache, revalidate only when stale.
  useEffect(() => {
    // The cache already painted during render, so this is the switch's first
    // data even when a revalidation follows in the background.
    if (perfView && readCacheValue<T>(key) !== null) perfViewPainted(perfView, "cache");
    void run("ensure");
  }, [run, perfView, key]);

  return {
    data,
    /** Blocking only while there is nothing to paint. */
    loading,
    /** A stale value is on screen while the background read runs. */
    revalidating,
    error,
    /** TTL-aware read, for polling and window-focus revalidation. */
    ensure: useCallback(() => run("ensure"), [run]),
    /** Unconditional read, for explicit refresh and post-mutation. */
    refresh: useCallback(() => run("force"), [run]),
    /** Optimistic write: the cache and the view move together. */
    mutate: useCallback(
      (next: T) => {
        writeCacheValue(key, next);
        setData(next);
      },
      [key],
    ),
  };
}
