import {
  hasSkillsCache,
  readSkillsCache,
  skillsCacheGeneration,
  writeSkillsCache,
  writeSkillsCacheStale,
} from "./skillsCache";
import type { SkillItem } from "./skills";
import { listSkills } from "./tauri";

const inflight = new Map<
  string,
  { generation: number; promise: Promise<SkillItem[]> }
>();

/**
 * One evaluation per scope, shared by the grid and the background preload.
 * A mutation bumps the cache generation, so a refresh starts a new read
 * instead of reusing the reply that was already on the way.
 * A late reply never overwrites an optimistic list; it is kept only when
 * the scope had nothing else to show, and then marked stale.
 */
export function loadSkillsScope(scope: string): Promise<SkillItem[]> {
  const pending = inflight.get(scope);
  const current = skillsCacheGeneration();
  if (pending && pending.generation === current) return pending.promise;

  const promise = listSkills(scope)
    .then((items) => {
      if (skillsCacheGeneration() !== current) {
        if (!hasSkillsCache(scope)) writeSkillsCacheStale(scope, items);
        return readSkillsCache(scope) ?? items;
      }
      writeSkillsCache(scope, items);
      return items;
    })
    .finally(() => {
      if (inflight.get(scope)?.promise === promise) inflight.delete(scope);
    });

  inflight.set(scope, { generation: current, promise });
  return promise;
}
