import { loadCacheValue, reloadCacheValue } from "./dataCache";
import { rtkStatus } from "./tauri";

/**
 * `rtk_status` shells out to `rtk --version`, so asking again on every visit
 * to the store tab pays a process spawn for an answer that only changes when
 * the operator installs or updates it.
 */
export const RTK_STATUS_KEY = "rtk:status";
export const RTK_STATUS_TTL_MS = 5 * 60_000;

/** Installed RTK version (or its path), `null` when it is not installed. */
export function ensureRtkStatus(): Promise<string | null> {
  return loadCacheValue({
    key: RTK_STATUS_KEY,
    ttlMs: RTK_STATUS_TTL_MS,
    load: rtkStatus,
  });
}

/** Re-probe after an install, update or uninstall. */
export function refreshRtkStatus(): Promise<string | null> {
  return reloadCacheValue({
    key: RTK_STATUS_KEY,
    ttlMs: RTK_STATUS_TTL_MS,
    load: rtkStatus,
  });
}
