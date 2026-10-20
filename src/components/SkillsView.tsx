import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Sparkles, Waypoints } from "lucide-react";
import {
  projectionPath,
  sortSkills,
  vaultPath,
  type SkillItem,
  type SortMode,
} from "../lib/skills";
import { GLOBAL_SCOPE } from "../lib/harnesses";
import { entryTokens } from "../lib/tokens";
import { markSkillsCacheStale, projectSkillState } from "../lib/skillsCache";
import { batchSetAll, deleteSkill as deleteVaultSkill, setSkillState } from "../lib/tauri";
import { devMockMode } from "../lib/fixtures";
import { useI18n } from "../hooks/useI18n";
import { useSkillsCache } from "../hooks/useSkillsCache";
import { useSkillStoreActions } from "../hooks/useSkillStoreActions";
import { ConfirmModal } from "./ConfirmModal";
import { SkillCard } from "./SkillCard";
import { EditSkillModal } from "./skills/EditSkillModal";
import { ImportSkillModal } from "./skills/ImportSkillModal";
import { FlatSkillGrid, GroupedSkillGrid } from "./SkillsGrid";
import { SkillScopeSelector } from "./SkillScopeSelector";
import { SkillsState, type SkillsStateKind } from "./SkillsStates";
import { SkillsToolbar } from "./SkillsToolbar";
import { OfficialStore } from "./OfficialStore";
import { StoreTabs } from "./StoreTabs";
import { ErrorBanner, SkeletonGrid } from "./ui";

interface SkillsViewProps {
  isTauri: boolean;
  /** Active skills per scope id, for the inner scope tabs. */
  harnessCounts: Record<string, number>;
  /** Catalog size; `0` hides the tab counters until the summary loads. */
  harnessTotal: number;
  /** Installed harness ids, or `null` while the bridge status is unknown. */
  installedHarnessIds: ReadonlySet<string> | null;
  query: string;
  onQueryChange: (query: string) => void;
  /** Bumped when something outside the grid invalidated the skill list. */
  reloadToken: number;
  /** Reports the header badge counters for the current scope. */
  onStatsChange: (stats: { active: number; total: number }) => void;
  /** Refreshes the shell's per-harness counters after a mutation. */
  onSummaryStale: () => void;
  /** Keeps the header refresh button spinning while the vault is rescanned. */
  onBusyChange: (busy: boolean) => void;
  /** Error raised outside the grid (for example by the bridge deploy). */
  externalError: string | null;
  onDismissExternalError: () => void;
  /** Shell toast for import success and other one-shot notices. */
  onToast?: (message: string) => void;
}

function toMessage(cause: unknown, fallback: string): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return fallback;
}

/**
 * The skill grid for one scope: stats, search results, ordering, bulk
 * projection and the per-card toggles that write through to the Rust core.
 *
 * The evaluated list comes from a per-scope cache, so switching harnesses
 * paints the new scope's cards in the same frame and revalidates behind the
 * user; only a scope that was never loaded shows the skeleton.
 *
 * The scope is local state, not a route: the harness tabs sit inside this view
 * exactly like the MCP registry's, so picking one re-evaluates the grid in
 * place instead of remounting the shell.
 */
export function SkillsView({
  isTauri,
  harnessCounts,
  harnessTotal,
  installedHarnessIds,
  query,
  onQueryChange,
  reloadToken,
  onStatsChange,
  onSummaryStale,
  onBusyChange,
  externalError,
  onDismissExternalError,
  onToast,
}: SkillsViewProps) {
  const { t } = useI18n();
  const [scope, setScope] = useState<string>(GLOBAL_SCOPE.id);

  const {
    skills,
    loading,
    refreshing,
    error,
    setError,
    updateSkills,
    reload,
    invalidate,
  } = useSkillsCache(scope, isTauri);

  const [importOpen, setImportOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deletingIds, setDeletingIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const [pendingIds, setPendingIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const [batchPending, setBatchPending] = useState(false);
  const [sortMode, setSortMode] = useState<SortMode>("active");
  const [showStore, setShowStore] = useState(false);
  // Highlight the card that just changed. `follow` scrolls to it only when it
  // was enabled; disabling must leave the viewport where the user is.
  const [flash, setFlash] = useState<{
    id: string;
    nonce: number;
    follow: boolean;
  } | null>(null);

  // Pending writes and a highlight belong to the scope that started them.
  const [shownScope, setShownScope] = useState(scope);
  if (shownScope !== scope) {
    setShownScope(scope);
    setPendingIds(new Set());
    setFlash(null);
  }

  // A new scope starts from a clean slate: no stale query or shell error.
  const selectScope = useCallback(
    (next: string) => {
      if (next === scope) return;

      setScope(next);
      onQueryChange("");
      onDismissExternalError();
    },
    [scope, onQueryChange, onDismissExternalError],
  );

  // The shell asks for a rescan by bumping the token (bridge deploy, refresh
  // button) instead of reaching into this component's state.
  const lastReloadToken = useRef(reloadToken);
  useEffect(() => {
    if (reloadToken === lastReloadToken.current) return;

    lastReloadToken.current = reloadToken;
    void invalidate();
  }, [reloadToken, invalidate]);

  const activeCount = skills.filter((skill) => skill.harnessEnabled).length;
  const disabledCount = skills.length - activeCount;
  const inheritedCount = skills.filter(
    (skill) => skill.inheritedFromGlobal,
  ).length;
  const savedTokens = skills.reduce(
    (sum, skill) =>
      skill.harnessEnabled
        ? sum
        : sum + entryTokens(skill.name, skill.description, skill.tokens),
    0,
  );

  useEffect(() => {
    onStatsChange({ active: activeCount, total: skills.length });
  }, [activeCount, skills.length, onStatsChange]);

  useEffect(() => {
    onBusyChange(loading || refreshing);
  }, [loading, refreshing, onBusyChange]);

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
    () =>
      isGrouped ? visibleSkills.filter((skill) => skill.harnessEnabled) : [],
    [isGrouped, visibleSkills],
  );
  const idleSkills = useMemo(
    () =>
      isGrouped ? visibleSkills.filter((skill) => !skill.harnessEnabled) : [],
    [isGrouped, visibleSkills],
  );

  // The grid reveals the moved card (it may be outside the rendered window);
  // this only retires the highlight.
  useEffect(() => {
    if (!flash) return;

    const timer = window.setTimeout(() => setFlash(null), 900);
    return () => window.clearTimeout(timer);
  }, [flash]);

  const toggleSkill = useCallback(
    async (id: string) => {
      const current = skills.find((skill) => skill.id === id);
      if (!current || current.inheritedFromGlobal) return;

      const nextEnabled = !current.harnessEnabled;
      const optimistic = projectSkillState(
        current,
        scope,
        nextEnabled,
        GLOBAL_SCOPE.id,
      );
      const globalScope = scope === GLOBAL_SCOPE.id;

      updateSkills((items) =>
        items.map((skill) => (skill.id === id ? optimistic : skill)),
      );
      setPendingIds((currentIds) => new Set(currentIds).add(id));
      setFlash((current) => ({
        id,
        nonce: (current?.nonce ?? 0) + 1,
        follow: nextEnabled,
      }));
      setError(null);

      try {
        await setSkillState(id, scope, nextEnabled);
        // A Global toggle changes what every harness inherits.
        if (globalScope) markSkillsCacheStale(scope);
        onSummaryStale();
      } catch (cause) {
        const message = t.skills.toggleFailed(
          nextEnabled ? t.skills.enableVerb : t.skills.disableVerb,
          current.name,
          toMessage(cause, t.common.unexpected),
        );
        updateSkills((items) =>
          items.map((skill) => (skill.id === id ? current : skill)),
        );
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
    [t, scope, skills, updateSkills, reload, setError, onSummaryStale],
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
    [t, isTauri, scope, skills.length, updateSkills, reload, setError, onSummaryStale],
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
  }, [pendingDeleteSkill, t, invalidate, onSummaryStale, onToast, setError]);

  const { statusOf: storeStatusOf, actionFor: storeActionFor } =
    useSkillStoreActions(skills, {
      scope,
      toggleSkill,
      invalidate,
      onSummaryStale,
      onToast,
    });

  const editingSkill = editingId
    ? (skills.find((skill) => skill.id === editingId) ?? null)
    : null;

  const renderSkillCard = (skill: SkillItem) => (
    <SkillCard
      key={skill.id}
      skill={skill}
      onToggle={(id) => void toggleSkill(id)}
      onEdit={(id) => setEditingId(id)}
      onDelete={(id) => requestDeleteSkill(id)}
      busy={batchPending || pendingIds.has(skill.id)}
      deleteBusy={deletingIds.has(skill.id)}
      flash={flash?.id === skill.id}
    />
  );

  const hasSkills = skills.length > 0;
  const canBatch = (isTauri || devMockMode()) && hasSkills && !loading && !batchPending;
  const banner = error ?? externalError;

  const stateKind: SkillsStateKind | null = (!isTauri && !devMockMode())
    ? "unavailable"
    : error && !hasSkills
      ? "read-error"
      : !hasSkills
        ? "empty-vault"
        : filteredSkills.length === 0
          ? "no-match"
          : null;

  const projectionLabel =
    scope === GLOBAL_SCOPE.id
      ? `${projectionPath}/global`
      : `${projectionPath}/${scope}`;

  return (
    <div className="mx-auto w-full max-w-[1400px]">
      <SkillScopeSelector
        scope={scope}
        onSelect={selectScope}
        counts={harnessCounts}
        total={harnessTotal}
        installedIds={installedHarnessIds}
      />

      <StoreTabs showStore={showStore} onChange={setShowStore} />

      <SkillsToolbar
        scope={scope}
        activeCount={activeCount}
        totalCount={skills.length}
        disabledCount={disabledCount}
        inheritedCount={inheritedCount}
        savedTokens={savedTokens}
        visibleCount={visibleSkills.length}
        sortMode={sortMode}
        onSortModeChange={setSortMode}
        canBatch={canBatch}
        batchPending={batchPending}
        onApplyAll={(enabled) => void applyAll(enabled)}
        onImport={() => setImportOpen(true)}
      />

      {banner && hasSkills && (
        <ErrorBanner
          message={banner}
          onDismiss={() => {
            setError(null);
            onDismissExternalError();
          }}
        />
      )}

      {/* Only a scope that has never been evaluated shows the skeleton. */}
      {!showStore && (loading && !hasSkills ? (
        <SkeletonGrid />
      ) : stateKind ? (
        <SkillsState
          kind={stateKind}
          error={error}
          query={query}
          onRescan={() => void reload()}
          onClearQuery={() => onQueryChange("")}
        />
      ) : isGrouped ? (
        <GroupedSkillGrid
          activeSkills={activeSkills}
          idleSkills={idleSkills}
          renderCard={renderSkillCard}
          focusKey={flash?.follow ? flash.id : null}
        />
      ) : (
        <FlatSkillGrid
          skills={visibleSkills}
          renderCard={renderSkillCard}
          focusKey={flash?.follow ? flash.id : null}
        />
      ))}

      {showStore && (
        <OfficialStore
          kind="skill"
          statusOf={storeStatusOf}
          actionFor={storeActionFor}
          projectScope={scope}
        />
      )}

      {editingSkill && (
        <EditSkillModal
          skillId={editingSkill.id}
          skillName={editingSkill.name}
          onClose={() => setEditingId(null)}
          onSaved={(name) => {
            onSummaryStale();
            void invalidate();
            onToast?.(t.skills.editSaved(name));
          }}
          onError={(message) => setError(message)}
        />
      )}

      {importOpen && (
        <ImportSkillModal
          isTauri={isTauri}
          onClose={() => setImportOpen(false)}
          onError={(message) => setError(message)}
          onImported={(skill) => {
            onSummaryStale();
            void invalidate();
            onDismissExternalError();
            setError(null);
            onToast?.(t.skills.import.success(skill.name));
          }}
        />
      )}

      {pendingDeleteSkill && (
        <ConfirmModal
          title={t.skills.deleteTitle}
          message={t.skills.deleteConfirm(pendingDeleteSkill.name)}
          confirmLabel={t.common.delete}
          cancelLabel={t.skills.deleteCancel}
          tone="danger"
          onCancel={() => setPendingDeleteSkill(null)}
          onConfirm={() => void confirmDeleteSkill()}
        />
      )}

      <div className="mt-4 flex flex-wrap items-center gap-2 text-[10.5px] text-zinc-600">
        <Waypoints className="size-3 shrink-0" />
        <span>
          {t.skills.symlinkedFrom}{" "}
          <code className="font-mono text-zinc-500">{vaultPath}</code>{" "}
          {t.skills.into}{" "}
          <code className="font-mono text-zinc-500">{projectionLabel}</code>.
        </span>
        <span className="flex items-center gap-1 text-zinc-700">
          <Sparkles className="size-3" />
          {t.skills.lockedNote}
        </span>
      </div>
    </div>
  );
}
