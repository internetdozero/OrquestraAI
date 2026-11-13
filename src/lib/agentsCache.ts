import { loadCacheValue, readCacheValue, reloadCacheValue, writeCacheValue } from "./dataCache";
import { readMasterAgentsMd } from "./tauri";

/**
 * `AGENTS.md` is edited from this app or by an agent on disk. A quarter of an
 * hour bounds how stale a hand edit can look, and every save in the UI writes
 * through the cache immediately.
 */
export const AGENTS_TEXT_KEY = "agents:text";
export const AGENTS_TEXT_TTL_MS = 15 * 60_000;

/** Master instructions already read this session, or `null` if still unknown. */
export function readAgentsCache(): string | null {
  return readCacheValue<string>(AGENTS_TEXT_KEY);
}

/** Remember a saved draft so remounting the editor does not reload it. */
export function writeAgentsCache(next: string): void {
  writeCacheValue(AGENTS_TEXT_KEY, next);
}

/** One read of `AGENTS.md`, shared with the background preload. */
export function ensureAgentsText(): Promise<string> {
  return loadCacheValue({
    key: AGENTS_TEXT_KEY,
    ttlMs: AGENTS_TEXT_TTL_MS,
    load: readMasterAgentsMd,
  });
}

/** Read `AGENTS.md` again, for an explicit recheck after an external edit. */
export function refreshAgentsText(): Promise<string> {
  return reloadCacheValue({
    key: AGENTS_TEXT_KEY,
    ttlMs: AGENTS_TEXT_TTL_MS,
    load: readMasterAgentsMd,
  });
}
