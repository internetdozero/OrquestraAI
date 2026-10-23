import { listOfficialStore, type OfficialStore } from "./tauri";

let catalog: OfficialStore | null = null;
let inflight: Promise<OfficialStore> | null = null;

/** Last store payload fetched, or `null` before the first read. */
export function readStoreCache(): OfficialStore | null {
  return catalog;
}

/** Drop the payload so the next read talks to the core again. */
export function invalidateStoreCache(): void {
  catalog = null;
  inflight = null;
}

/** Resolve the shared store payload, joining a read that is already on the way. */
export function ensureStoreCatalog(): Promise<OfficialStore> {
  if (catalog) return Promise.resolve(catalog);
  if (inflight) return inflight;

  inflight = listOfficialStore()
    .then((store) => {
      catalog = store;
      return store;
    })
    .finally(() => {
      inflight = null;
    });

  return inflight;
}

/** Forget the cached payload and read it again. */
export function refreshStoreCatalog(): Promise<OfficialStore> {
  invalidateStoreCache();
  return ensureStoreCatalog();
}
