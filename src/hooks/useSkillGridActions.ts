import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import { sortSkills, type SkillItem, type SortMode } from "../lib/skills";
import { GLOBAL_SCOPE } from "../lib/harnesses";
import { markSkillsCacheStale, projectSkillState } from "../lib/skillsCache";
import { batchSetAll, deleteSkill as deleteVaultSkill, setSkillState } from "../lib/tauri";
import type { useI18n } from "./useI18n";
import type { useSkillsCache } from "./useSkillsCache";

type Translate = ReturnType<typeof useI18n>["t"];
type Cache = ReturnType<typeof useSkillsCache>;

type Flash = { id: string; nonce: number; follow: boolean } | null;

function toMessage(cause: unknown, fallback: string): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return fallback;
}

/** Filter, sort and the optimistic toggle / batch / delete writes for the skill grid. */
export function useSkillGridActions(input: {
  query: string;
  skills: SkillItem[];
  sortMode: SortMode;
  flash: Flash;
  setFlash: Dispatch<SetStateAction<Flash>>;
  scope: string;
  updateSkills: Cache["updateSkills"];
  reload: Cache["reload"];
  setError: Cache["setError"];
  onSummaryStale: () => void;
  isTauri: boolean;
  t: Translate;
  setPendingIds: Dispatch<SetStateAction<ReadonlySet<string>>>;
  setBatchPending: Dispatch<SetStateAction<boolean>>;
  setDeletingIds: Dispatch<SetStateAction<ReadonlySet<string>>>;
  invalidate: Cache["invalidate"];
  onToast?: (message: string) => void;
}) {
  const {
    query,
    skills,
    sortMode,
    flash,
    setFlash,
    scope,
    updateSkills,
    reload,
    setError,
    onSummaryStale,
    isTauri,
    t,
    setPendingIds,
    setBatchPending,
    setDeletingIds,
    invalidate,
    onToast,
  } = input;

  const filteredSkills = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return skills;

    return skills.filter((skill) =>
      [skill.name, skill.id, skill.category, skill.description].some((field) =>
        field.toLowerCase().includes(needle),
      ),
    );
  }, [query, skills]);

  // `toggleSkill` updates the list optimistically, so a card that was just
  // enabled is already in this list's active block on the very next render —
  // before the Rust command that creates the symlink has even resolved.
  const visibleSkills = useMemo(
    () => sortSkills(filteredSkills, sortMode),
    [filteredSkills, sortMode],
  );

  const isGrouped = sortMode === "active";
  const activeSkills = useMemo(
    () => (isGrouped ? visibleSkills.filter((skill) => skill.harnessEnabled) : []),
    [isGrouped, visibleSkills],
  );
  const idleSkills = useMemo(
    () => (isGrouped ? visibleSkills.filter((skill) => !skill.harnessEnabled) : []),
    [isGrouped, visibleSkills],
  );

  useEffect(() => {
    if (!flash) return;

    const timer = window.setTimeout(() => setFlash(null), 900);
    return () => window.clearTimeout(timer);
  }, [flash, setFlash]);

  const toggleSkill = useCallback(
    async (id: string) => {
      const current = skills.find((skill) => skill.id === id);
      if (!current || current.inheritedFromGlobal) return;

      const nextEnabled = !current.harnessEnabled;
      const optimistic = projectSkillState(current, scope, nextEnabled, GLOBAL_SCOPE.id);
      const globalScope = scope === GLOBAL_SCOPE.id;

      updateSkills((items) => items.map((skill) => (skill.id === id ? optimistic : skill)));
      setPendingIds((currentIds) => new Set(currentIds).add(id));
      setFlash((currentFlash) => ({
        id,
        nonce: (currentFlash?.nonce ?? 0) + 1,
        follow: nextEnabled,
      }));
      setError(null);

      try {
        await setSkillState(id, scope, nextEnabled);
        if (globalScope) markSkillsCacheStale(scope);
        onSummaryStale();
      } catch (cause) {
        const message = t.skills.toggleFailed(
          nextEnabled ? t.skills.enableVerb : t.skills.disableVerb,
          current.name,
          toMessage(cause, t.common.unexpected),
        );
        updateSkills((items) => items.map((skill) => (skill.id === id ? current : skill)));
        await reload(true);
        setError(message);
      } finally {
        setPendingIds((currentIds) => {
          const remaining = new Set(currentIds);
          remaining.delete(id);
          return remaining;
        });
      }
    },
    [t, scope, skills, updateSkills, reload, setError, onSummaryStale, setFlash, setPendingIds],
  );

  const applyAll = useCallback(
    async (enabled: boolean) => {
      if (!isTauri || skills.length === 0) return;

      setBatchPending(true);
      setError(null);
      updateSkills((items) =>
        items.map((skill) =>
          scope !== GLOBAL_SCOPE.id && skill.inheritedFromGlobal
            ? skill
            : projectSkillState(skill, scope, enabled, GLOBAL_SCOPE.id),
        ),
      );

      try {
        await batchSetAll(scope, enabled);
        if (scope === GLOBAL_SCOPE.id) markSkillsCacheStale(scope);
        onSummaryStale();
      } catch (cause) {
        const message = t.skills.batchFailed(toMessage(cause, t.common.unexpected));
        await reload(true);
        setError(message);
      } finally {
        setBatchPending(false);
      }
    },
    [
      t,
      isTauri,
      scope,
      skills.length,
      updateSkills,
      reload,
      setError,
      onSummaryStale,
      setBatchPending,
    ],
  );

  const [pendingDeleteSkill, setPendingDeleteSkill] = useState<SkillItem | null>(null);

  const requestDeleteSkill = useCallback(
    (id: string) => {
      const current = skills.find((skill) => skill.id === id);
      if (!current || !isTauri) return;
      setPendingDeleteSkill(current);
    },
    [skills, isTauri],
  );

  const confirmDeleteSkill = useCallback(async () => {
    const current = pendingDeleteSkill;
    if (!current) return;
    setPendingDeleteSkill(null);
    setDeletingIds((ids) => new Set(ids).add(current.id));
    try {
      await deleteVaultSkill(current.id);
      onSummaryStale();
      await invalidate();
      onToast?.(t.skills.deleteDeleted(current.name));
    } catch (cause) {
      setError(t.skills.deleteFailed(toMessage(cause, t.common.unexpected)));
    } finally {
      setDeletingIds((ids) => {
        const remaining = new Set(ids);
        remaining.delete(current.id);
        return remaining;
      });
    }
  }, [pendingDeleteSkill, t, invalidate, onSummaryStale, onToast, setError, setDeletingIds]);

  return {
    filteredSkills,
    visibleSkills,
    isGrouped,
    activeSkills,
    idleSkills,
    toggleSkill,
    applyAll,
    pendingDeleteSkill,
    setPendingDeleteSkill,
    requestDeleteSkill,
    confirmDeleteSkill,
  };
}
