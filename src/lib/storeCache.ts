import { invalidateCache, loadCacheValue, readCacheValue, reloadCacheValue } from "./dataCache";
import { listOfficialStore, type OfficialStore } from "./tauri";

/**
 * The official store is a catalog published with releases, so it barely
 * changes during a session; 10 min still lets a newly published entry appear
 * without restarting, and every install/sync path invalidates it.
 */
export const STORE_CATALOG_KEY = "store:catalog";
export const STORE_CATALOG_TTL_MS = 10 * 60_000;

/** Last store payload fetched, or `null` before the first read. */
export function readStoreCache(): OfficialStore | null {
  return readCacheValue<OfficialStore>(STORE_CATALOG_KEY);
}

/** Drop the payload so the next read talks to the core again. */
export function invalidateStoreCache(): void {
  invalidateCache(STORE_CATALOG_KEY);
}

/** Resolve the shared store payload, joining a read that is already on the way. */
export function ensureStoreCatalog(): Promise<OfficialStore> {
  return loadCacheValue({
    key: STORE_CATALOG_KEY,
    ttlMs: STORE_CATALOG_TTL_MS,
    load: listOfficialStore,
  });
}

/** Forget the cached payload and read it again. */
export function refreshStoreCatalog(): Promise<OfficialStore> {
  return reloadCacheValue({
    key: STORE_CATALOG_KEY,
    ttlMs: STORE_CATALOG_TTL_MS,
    load: listOfficialStore,
  });
}
