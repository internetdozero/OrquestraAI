import { useCallback, useEffect, useRef, useState } from "react";
import { useI18n } from "./useI18n";
import type { SkillItem } from "../lib/skills";
import {
  hasSkillsCache,
  isSkillsCacheStale,
  markSkillsCacheStale,
  readSkillsCache,
  writeSkillsCache,
} from "../lib/skillsCache";
import { loadSkillsScope } from "../lib/skillsLoader";
import { devMockMode } from "../lib/fixtures";

function toMessage(cause: unknown, fallback: string): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return fallback;
}

/**
 * Skills for one scope, cache first.
 *
 * Switching scope adopts that scope's cached evaluation during the render
 * itself, so no frame ever shows the previous scope's cards, an empty grid or
 * a skeleton. A background revalidate runs on every visit and repaints in
 * place; a scope that has never been loaded takes the normal loading path.
 *
 * Responses are matched to the request that asked for them: a late reply for a
 * scope the user has already left still refreshes that scope's cache, but
 * never paints over the current one.
 */
export function useSkillsCache(scope: string, isTauri: boolean) {
  const { t } = useI18n();
  const [skills, setSkills] = useState<SkillItem[]>(
    () => readSkillsCache(scope) ?? [],
  );
  const [loading, setLoading] = useState(() => !hasSkillsCache(scope));
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);

  // Latest list, for the synchronous optimistic writes below.
  const skillsRef = useRef(skills);
  skillsRef.current = skills;

  const [shownScope, setShownScope] = useState(scope);
  if (shownScope !== scope) {
    setShownScope(scope);
    setSkills(readSkillsCache(scope) ?? []);
    setLoading(!hasSkillsCache(scope));
    setError(null);
  }

  const reload = useCallback(
    async (silent = false) => {
      requestRef.current += 1;
      const request = requestRef.current;

      if (!isTauri && !devMockMode()) {
        setLoading(false);
        return;
      }

      if (!silent) setRefreshing(true);
      try {
        const items = await loadSkillsScope(scope);
        if (request !== requestRef.current) return;
        setSkills(items);
        setError(null);
      } catch (cause) {
        if (request !== requestRef.current) return;
        setError(toMessage(cause, t.common.unexpected));
      } finally {
        if (request === requestRef.current) {
          setLoading(false);
          setRefreshing(false);
        }
      }
    },
    [isTauri, scope, t],
  );

  // A fresh cache paints as-is. Only a missing or stale scope hits the core,
  // and a stale one keeps its cards up while that read finishes.
  useEffect(() => {
    if (hasSkillsCache(scope) && !isSkillsCacheStale(scope)) {
      setLoading(false);
      return;
    }
    void reload(hasSkillsCache(scope));
  }, [reload, scope]);

  /** Optimistic write: the list and its cache entry move together. */
  const updateSkills = useCallback(
    (updater: (items: SkillItem[]) => SkillItem[]) => {
      const next = updater(skillsRef.current);
      skillsRef.current = next;
      writeSkillsCache(scope, next);
      setSkills(next);
    },
    [scope],
  );

  /**
   * Vault rescan or bridge deploy: projections may have changed for any
   * harness, so no cached scope can be trusted. The visible list stays put
   * while the fresh one loads — a rescan should not blank the grid either.
   */
  const invalidate = useCallback(async () => {
    markSkillsCacheStale();
    await reload(false);
  }, [reload]);

  return {
    skills,
    loading,
    refreshing,
    error,
    setError,
    updateSkills,
    reload,
    invalidate,
  };
}
