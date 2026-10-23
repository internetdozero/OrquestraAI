import { useCallback, useEffect, useRef, useState } from "react";
import { Sparkles, Waypoints } from "lucide-react";
import { projectionPath, vaultPath, type SkillItem, type SortMode } from "../lib/skills";
import { GLOBAL_SCOPE } from "../lib/harnesses";
import { entryTokens } from "../lib/tokens";
import { devMockMode } from "../lib/fixtures";
import { useI18n } from "../hooks/useI18n";
import { useDebouncedValue } from "../hooks/useDebouncedValue";
import { useSkillsCache } from "../hooks/useSkillsCache";
import { useSkillGridActions } from "../hooks/useSkillGridActions";
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

  const { skills, loading, refreshing, error, setError, updateSkills, reload, invalidate } =
    useSkillsCache(scope, isTauri);

  const [importOpen, setImportOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deletingIds, setDeletingIds] = useState<ReadonlySet<string>>(() => new Set());
  const [pendingIds, setPendingIds] = useState<ReadonlySet<string>>(() => new Set());
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
  const inheritedCount = skills.filter((skill) => skill.inheritedFromGlobal).length;
  const savedTokens = skills.reduce(
    (sum, skill) =>
      skill.harnessEnabled ? sum : sum + entryTokens(skill.name, skill.description, skill.tokens),
    0,
  );

  useEffect(() => {
    onStatsChange({ active: activeCount, total: skills.length });
  }, [activeCount, skills.length, onStatsChange]);

  useEffect(() => {
    onBusyChange(loading || refreshing);
  }, [loading, refreshing, onBusyChange]);

  // Debounced filter text: the header input stays responsive per keystroke
  // while the grid re-evaluates once per pause, and memoized cards skip
  // every card whose props did not change. Clearing snaps back instantly.
  const debouncedQuery = useDebouncedValue(query, 200);
  const activeQuery = query === "" ? "" : debouncedQuery;

  const {
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
  } = useSkillGridActions({
    query: activeQuery,
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
  });

  const { statusOf: storeStatusOf, actionFor: storeActionFor } = useSkillStoreActions(skills, {
    scope,
    toggleSkill,
    invalidate,
    onSummaryStale,
    onToast,
  });

  const editingSkill = editingId ? (skills.find((skill) => skill.id === editingId) ?? null) : null;

  // Stable callback identities: SkillCard is memoized and compares props by
  // reference, so inline arrows here would re-render every card per keystroke.
  const handleToggleSkill = useCallback((id: string) => void toggleSkill(id), [toggleSkill]);
  const handleEditSkill = useCallback((id: string) => setEditingId(id), []);
  const handleDeleteSkill = useCallback(
    (id: string) => void requestDeleteSkill(id),
    [requestDeleteSkill],
  );

  const renderSkillCard = useCallback(
    (skill: SkillItem) => (
      <SkillCard
        key={skill.id}
        skill={skill}
        onToggle={handleToggleSkill}
        onEdit={handleEditSkill}
        onDelete={handleDeleteSkill}
        busy={batchPending || pendingIds.has(skill.id)}
        deleteBusy={deletingIds.has(skill.id)}
        flash={flash?.id === skill.id}
      />
    ),
    [
      handleToggleSkill,
      handleEditSkill,
      handleDeleteSkill,
      batchPending,
      pendingIds,
      deletingIds,
      flash,
    ],
  );

  const hasSkills = skills.length > 0;
  const canBatch = (isTauri || devMockMode()) && hasSkills && !loading && !batchPending;
  const banner = error ?? externalError;

  const stateKind: SkillsStateKind | null =
    !isTauri && !devMockMode()
      ? "unavailable"
      : error && !hasSkills
        ? "read-error"
        : !hasSkills
          ? "empty-vault"
          : filteredSkills.length === 0
            ? "no-match"
            : null;

  const projectionLabel =
    scope === GLOBAL_SCOPE.id ? `${projectionPath}/global` : `${projectionPath}/${scope}`;

  return (
    <div className="mx-auto w-full max-w-[1400px]">
      <SkillScopeSelector
        scope={scope}
        onSelect={selectScope}
        counts={harnessCounts}
        total={harnessTotal}
        installedIds={installedHarnessIds}
      />

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
        showStore={showStore}
        onShowStoreChange={setShowStore}
        query={query}
        onQueryChange={onQueryChange}
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
      {!showStore &&
        (loading && !hasSkills ? (
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
          {t.skills.symlinkedFrom} <code className="font-mono text-zinc-500">{vaultPath}</code>{" "}
          {t.skills.into} <code className="font-mono text-zinc-500">{projectionLabel}</code>.
        </span>
        <span className="flex items-center gap-1 text-zinc-700">
          <Sparkles className="size-3" />
          {t.skills.lockedNote}
        </span>
      </div>
    </div>
  );
}
