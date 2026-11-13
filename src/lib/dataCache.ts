/**
 * Shared resource cache for IPC-backed data: TTL, single-flight and
 * stale-while-revalidate in one place.
 *
 * Every view in this app is mounted on demand (the shell renders exactly one),
 * so a tab switch used to look like a cold start: a fresh hook instance with no
 * state, an unconditional `useEffect` read and a skeleton. The ad-hoc caches
 * that grew around that (`ideasCache`, `vaultCache`, …) dedupe an in-flight read
 * but never expire, so the other half of the problem appeared instead — data
 * that is repainted for a whole session without ever being revalidated.
 *
 * This module fixes both ends with one rule set:
 *
 * - `loadCacheValue` answers from memory while the entry is younger than the
 *   caller's TTL, joins a read that already started, and otherwise reads once.
 * - `reloadCacheValue` always reads (still joining an in-flight one), for
 *   explicit refreshes and post-mutation invalidation.
 * - `writeCacheValue` stores an optimistic result and bumps the generation, so
 *   a reply that started before the write can never overwrite it.
 *
 * TTLs belong to the caller and should match the data, not the screen: the
 * session catalog is an expensive disk walk (hundreds of ms, seconds on a
 * worktree-heavy install) and tolerates ~30 s of staleness, while the retention
 * policy changes only when the operator saves it.
 */

interface Entry {
  data: unknown;
  /** `false` until a read stored something: `data` may legitimately be `null`. */
  hasData: boolean;
  /** When `data` was stored (ms epoch). */
  at: number;
  /** Bumped by every write and invalidation; replies from before it are dropped. */
  generation: number;
  inflight: Promise<unknown> | null;
}

const entries = new Map<string, Entry>();
const generations = new Map<string, number>();

/** One cache read: the key, how long it stays fresh, and how to fill it. */
export interface CacheRequest<T> {
  key: string;
  ttlMs: number;
  load: () => Promise<T>;
}

function generationOf(key: string): number {
  return generations.get(key) ?? 0;
}

/** Cached value, or `null` when this key was never filled. */
export function readCacheValue<T>(key: string): T | null {
  const entry = entries.get(key);
  return entry?.hasData ? (entry.data as T) : null;
}

/** Age of the cached value in ms, or `null` when nothing is cached. */
export function cacheAgeMs(key: string): number | null {
  const entry = entries.get(key);
  return entry?.hasData ? Date.now() - entry.at : null;
}

/** Is the value fresh enough to serve without a background read? */
export function isCacheFresh(key: string, ttlMs: number): boolean {
  const entry = entries.get(key);
  if (!entry?.hasData) return false;
  // A read is already on the way: treat it as fresh so callers do not stack one.
  if (entry.inflight) return true;
  return Date.now() - entry.at < ttlMs;
}

/**
 * Store a value now and discard any read that started earlier.
 *
 * Optimistic writes go through here so the UI never flips back to the previous
 * state when a reply that predates the write lands.
 */
export function writeCacheValue<T>(key: string, data: T): void {
  generations.set(key, generationOf(key) + 1);
  const entry = entries.get(key);
  entries.set(key, {
    data,
    hasData: true,
    at: Date.now(),
    generation: generationOf(key),
    inflight: entry?.inflight ?? null,
  });
}

/** Forget one key; the next read starts from the core again. */
export function invalidateCache(key: string): void {
  generations.set(key, generationOf(key) + 1);
  entries.delete(key);
}

function startRequest<T>({ key, load }: CacheRequest<T>): Promise<T> {
  const generation = generationOf(key);
  const previous = entries.get(key);
  const pending = load()
    .then((data) => {
      // A mutation (or an invalidation) landed while this read was open.
      if (generationOf(key) !== generation) return data;
      entries.set(key, {
        data,
        hasData: true,
        at: Date.now(),
        generation,
        inflight: null,
      });
      return data;
    })
    .finally(() => {
      const current = entries.get(key);
      if (current && current.inflight === pending) current.inflight = null;
    });

  entries.set(key, {
    data: previous?.data,
    hasData: previous?.hasData ?? false,
    at: previous?.at ?? 0,
    generation,
    inflight: pending,
  });
  return pending;
}

/** Fresh cache → memory; otherwise one read shared by every caller. */
export function loadCacheValue<T>(request: CacheRequest<T>): Promise<T> {
  const entry = entries.get(request.key);
  if (entry?.inflight) return entry.inflight as Promise<T>;
  if (entry?.hasData && Date.now() - entry.at < request.ttlMs) {
    return Promise.resolve(entry.data as T);
  }
  return startRequest(request);
}

/** Always read (joining a read already in flight), then store the reply. */
export function reloadCacheValue<T>(request: CacheRequest<T>): Promise<T> {
  const entry = entries.get(request.key);
  if (entry?.inflight) return entry.inflight as Promise<T>;
  return startRequest(request);
}
