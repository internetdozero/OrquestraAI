import type { SkillItem } from "./skills";

/**
 * Per-scope cache of evaluated skill lists.
 *
 * The skill catalog is identical across harness scopes — only `harnessEnabled`
 * and `inheritedFromGlobal` differ — so a scope switch is a re-evaluation, not
 * a cold read. Keeping the last evaluation per scope lets the grid paint
 * immediately and revalidate in the background (stale-while-revalidate)
 * instead of blanking to a skeleton on every tab click.
 *
 * Entries are served until something explicitly invalidates them:
 * - a toggle or batch action rewrites the scope it ran in;
 * - a Global toggle marks every harness stale, because inheritance changed;
 * - a vault rescan or bridge deploy drops everything.
 */
export interface CachedSkills {
  items: SkillItem[];
  /** Serve now, but revalidate when the scope is next shown. */
  stale: boolean;
}

const cache = new Map<string, CachedSkills>();

/**
 * Optimistic projection of one skill after a toggle in `scope`, mirroring the
 * Rust rules: a Global toggle also carries into the harness projection, while
 * a harness toggle only ever moves that harness's own flag. The cache stores
 * projected lists, so an optimistic write goes through here too.
 */
export function projectSkillState(
  skill: SkillItem,
  scope: string,
  enabled: boolean,
  globalScopeId: string,
): SkillItem {
  if (scope !== globalScopeId) return { ...skill, harnessEnabled: enabled };

  return {
    ...skill,
    globalEnabled: enabled,
    harnessEnabled: enabled,
    inheritedFromGlobal: false,
  };
}

/** Cached list for `scope`, or `null` when this scope was never loaded. */
export function readSkillsCache(scope: string): SkillItem[] | null {
  return cache.get(scope)?.items ?? null;
}

export function hasSkillsCache(scope: string): boolean {
  return cache.has(scope);
}

export function isSkillsCacheStale(scope: string): boolean {
  return cache.get(scope)?.stale ?? false;
}

/** Store an evaluated list; it is fresh until something marks it stale. */
export function writeSkillsCache(scope: string, items: SkillItem[]): void {
  cache.set(scope, { items, stale: false });
}

/**
 * Keep every cached scope servable but due for revalidation. `exceptScope`
 * stays fresh — used when only the other scopes can be affected.
 */
export function markSkillsCacheStale(exceptScope?: string): void {
  for (const [scope, entry] of cache) {
    if (scope === exceptScope) continue;
    entry.stale = true;
  }
}

/** Drop one scope's entry, or the whole cache when called without a scope. */
export function dropSkillsCache(scope?: string): void {
  if (scope === undefined) {
    cache.clear();
    return;
  }
  cache.delete(scope);
}
