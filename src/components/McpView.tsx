import { useCallback, useEffect, useMemo, useState } from "react";
import { AlertTriangle, Loader2, Plug, Plus, Unplug, Waypoints } from "lucide-react";
import { coversHarness, sortMcps, type McpServer, type McpSortMode } from "../lib/mcps";
import { useI18n } from "../hooks/useI18n";
import { useDebouncedValue } from "../hooks/useDebouncedValue";
import { devMockMode } from "../lib/fixtures";
import { useMcpCatalog } from "../hooks/useMcpCatalog";
import { useMcpMutations } from "../hooks/useMcpMutations";
import { useMcpStoreActions } from "../hooks/useMcpStoreActions";
import { GLOBAL_SCOPE } from "../lib/harnesses";
import { sumMcpTokens } from "../lib/tokens";
import { formatTokens } from "../lib/utils";
import { McpCard } from "./McpCard";
import { McpListRow } from "./McpListRow";
import { McpEmptyState } from "./McpEmptyState";
import { StatePanel } from "./ui";
import { McpDetailDrawer } from "./McpDetailDrawer";
import { McpFormModal } from "./McpFormModal";
import { McpScopeSelector } from "./McpScopeSelector";
import { McpToolbar } from "./McpToolbar";
import { VirtualCardGrid } from "./VirtualCardGrid";
import { OfficialStore } from "./OfficialStore";
import { StoreTabs } from "./StoreTabs";

interface McpViewProps {
  isTauri: boolean;
  onToast: (message: string) => void;
  /** Keeps the sidebar badge in step with the catalog. */
  onActiveCountChange?: (count: number) => void;
  /** Bumped when something outside the view (a preset) changed the catalog. */
  reloadToken?: number;
  /** Live search text owned by the header input. */
  query?: string;
  onQueryChange?: (query: string) => void;
}

export function McpView({
  isTauri,
  onToast,
  onActiveCountChange,
  reloadToken = 0,
  query = "",
  onQueryChange,
}: McpViewProps) {
  const { t } = useI18n();
  const [mockMode] = useState(devMockMode);
  const { servers, targets, loading, error, setError, load, replaceServers } = useMcpCatalog(
    isTauri,
    reloadToken,
  );
  const [scope, setScope] = useState<string>(GLOBAL_SCOPE.id);
  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<McpServer | null>(null);
  const [sortMode, setSortMode] = useState<McpSortMode>("active");
  // Server whose execution details are open in the inspector drawer.
  const [inspecting, setInspecting] = useState<McpServer | null>(null);
  const [view, setView] = useState<"grid" | "list">(() =>
    localStorage.getItem("mcps-view") === "list" ? "list" : "grid",
  );
  const [showStore, setShowStore] = useState(false);

  const activeServers = useMemo(
    () => servers.filter((server) => server.globalEnabled || server.enabledHarnesses.length > 0),
    [servers],
  );

  const scopeTokens = useMemo(
    () => sumMcpTokens(servers, (server) => coversHarness(server, scope)),
    [servers, scope],
  );

  useEffect(() => {
    onActiveCountChange?.(activeServers.length);
  }, [activeServers.length, onActiveCountChange]);

  useEffect(() => {
    localStorage.setItem("mcps-view", view);
  }, [view]);

  const scopeLabel =
    scope === GLOBAL_SCOPE.id
      ? "Global"
      : (targets.find((target) => target.id === scope)?.label ?? scope);

  // Debounced filter text (mirrors SkillsView): the header input stays
  // responsive per keystroke while the grid re-evaluates once per pause.
  // Clearing snaps back instantly instead of waiting out the delay.
  const debouncedQuery = useDebouncedValue(query, 200);
  const activeQuery = query === "" ? "" : debouncedQuery;

  const filteredServers = useMemo(() => {
    const needle = activeQuery.trim().toLowerCase();
    if (!needle) return servers;
    return servers.filter((server) =>
      [server.name, server.id, server.description, server.command].some((field) =>
        field?.toLowerCase().includes(needle),
      ),
    );
  }, [activeQuery, servers]);

  const visibleServers = useMemo(
    () => sortMcps(filteredServers, sortMode, scope),
    [filteredServers, sortMode, scope],
  );

  const {
    busyIds,
    saving,
    formError,
    setFormError,
    batchPending,
    toggle,
    applyAll,
    remove,
    save,
  } = useMcpMutations({
    scope,
    scopeLabel,
    replaceServers,
    load,
    setError,
    onToast,
    onSaved: () => {
      setFormOpen(false);
      setEditing(null);
    },
  });

  // Stable callback identities: McpCard is memoized and compares props by
  // reference, so inline arrows in renderItem would re-render every card
  // per keystroke. toggle/remove are useCallback'd at the hook layer.
  const handleToggleServer = useCallback((server: McpServer) => void toggle(server), [toggle]);
  const handleInspectServer = useCallback((server: McpServer) => setInspecting(server), []);
  const handleEditServer = useCallback(
    (server: McpServer) => {
      setEditing(server);
      setFormError(null);
      setFormOpen(true);
    },
    [setFormError],
  );
  const handleDeleteServer = useCallback((server: McpServer) => void remove(server), [remove]);
  const renderServerRow = useCallback(
    (server: McpServer) => (
      <McpListRow
        server={server}
        scope={scope}
        scopeLabel={scopeLabel}
        busy={busyIds.has(server.id)}
        onToggle={handleToggleServer}
        onInspect={handleInspectServer}
        onEdit={handleEditServer}
        onDelete={handleDeleteServer}
      />
    ),
    [
      scope,
      scopeLabel,
      busyIds,
      handleToggleServer,
      handleInspectServer,
      handleEditServer,
      handleDeleteServer,
    ],
  );

  const { statusOf: storeStatusOf, actionFor: storeActionFor } = useMcpStoreActions(servers, {
    scope,
    toggle,
    save,
    reload: load,
    onToast,
  });

  if (!isTauri && !mockMode) {
    return (
      <div className="flex h-full items-center justify-center px-6">
        <StatePanel
          icon={Unplug}
          title={t.skills.stateUnavailableTitle}
          description={t.mcps.unavailableBody}
        >
          <code className="rounded border border-white/10 bg-white/[0.03] px-2 py-1 font-mono text-[10.5px] text-zinc-400">
            npm run tauri dev
          </code>
        </StatePanel>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col overflow-y-auto px-4 py-5 sm:px-6">
      <div className="mx-auto w-full max-w-[1400px]">
        {mockMode && (
          <p
            role="status"
            className="mb-3 rounded-lg border border-cyan-500/25 bg-cyan-500/[0.06] px-3 py-1.5 font-mono text-[10.5px] text-cyan-200/90"
          >
            {t.mcps.mockBanner}
          </p>
        )}
        <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="text-[10px] font-medium tracking-[0.14em] text-zinc-500 uppercase">
              {t.mcps.headerChip}
            </p>
            <h2 className="text-lg font-semibold tracking-tight text-zinc-100">
              {t.mcps.registryTitle}
            </h2>
            <p className="mt-0.5 max-w-2xl text-[11px] leading-relaxed text-zinc-500">
              {t.mcps.subtitle}
            </p>
            <p className="mt-1 font-mono text-[10.5px] text-zinc-600" title={t.tokens.scopeHint}>
              {t.tokens.entry} ≈ {formatTokens(scopeTokens.entry)} · {t.tokens.full} ≈{" "}
              {formatTokens(scopeTokens.full)} {t.tokens.unit}
            </p>
          </div>

          <div className="flex shrink-0 items-center gap-2">
            <div className="accent-border accent-bg-soft flex items-center gap-2 rounded-lg border px-3 py-2">
              <Plug className="accent-text size-3.5" />
              <span className="accent-text text-[11.5px] font-medium">{t.mcps.active}</span>
              <span className="accent-text font-mono text-[11.5px] opacity-70">
                {activeServers.length}/{servers.length}
              </span>
            </div>
            <button
              type="button"
              onClick={() => {
                setEditing(null);
                setFormError(null);
                setFormOpen(true);
              }}
              className="flex items-center gap-1.5 rounded-lg border border-emerald-500/30 bg-emerald-500/15 px-3 py-2 text-[11.5px] font-medium text-emerald-200 transition hover:bg-emerald-500/25"
            >
              <Plus className="size-3.5" />
              {t.mcps.addServer}
            </button>
          </div>
        </div>

        {/* The toggle writes to whichever scope is chosen here. */}
        <McpScopeSelector
          scope={scope}
          targets={targets}
          onSelect={(next) => {
            setScope(next);
            onQueryChange?.("");
          }}
        />

        <div className="mb-4 flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
          <StoreTabs showStore={showStore} onChange={setShowStore} className="mb-0" />
          <McpToolbar
            query={query}
            onQueryChange={(value) => onQueryChange?.(value)}
            scope={scope}
            scopeLabel={scopeLabel}
            visibleCount={visibleServers.length}
            totalCount={servers.length}
            sortMode={sortMode}
            onSortModeChange={setSortMode}
            canBatch={(isTauri || mockMode) && servers.length > 0 && !batchPending && !loading}
            batchPending={batchPending}
            onApplyAll={(enabled) => void applyAll(enabled, servers.length)}
            view={view}
            onViewChange={setView}
          />
        </div>

        {error && (
          <div
            role="alert"
            className="mb-4 flex items-start gap-2 rounded-lg border border-red-500/25 bg-red-500/[0.07] px-3 py-2 text-[11.5px] leading-relaxed text-red-200"
          >
            <AlertTriangle className="mt-px size-3.5 shrink-0" />
            <span className="min-w-0 flex-1">{error}</span>
            <button
              type="button"
              onClick={() => setError(null)}
              className="shrink-0 text-red-300/70 hover:text-red-200"
            >
              {t.common.dismiss}
            </button>
          </div>
        )}

        {!showStore &&
          (loading ? (
            <div className="flex items-center justify-center gap-2 py-16 text-[12px] text-zinc-500">
              <Loader2 className="size-4 animate-spin" />
              {t.mcps.reading}
            </div>
          ) : visibleServers.length === 0 ? (
            <McpEmptyState
              onExploreStore={() => setShowStore(true)}
              onAddManual={() => {
                setEditing(null);
                setFormError(null);
                setFormOpen(true);
              }}
            />
          ) : view === "list" ? (
            <VirtualCardGrid
              items={visibleServers}
              keyOf={(server) => server.id}
              className="flex flex-col gap-2"
              renderItem={renderServerRow}
            />
          ) : (
            <VirtualCardGrid
              items={visibleServers}
              keyOf={(server) => server.id}
              className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3"
              renderItem={(server) => (
                <McpCard
                  server={server}
                  scope={scope}
                  scopeLabel={scopeLabel}
                  busy={busyIds.has(server.id)}
                  onToggle={handleToggleServer}
                  onInspect={handleInspectServer}
                  onEdit={handleEditServer}
                  onDelete={handleDeleteServer}
                />
              )}
            />
          ))}

        {showStore && (
          <OfficialStore kind="mcp" statusOf={storeStatusOf} actionFor={storeActionFor} />
        )}

        <div className="mt-4 flex flex-wrap items-center gap-2 text-[10.5px] text-zinc-600">
          <Waypoints className="size-3 shrink-0" />
          <span>
            {t.mcps.targets}{" "}
            {targets.length === 0
              ? t.mcps.noAdapters
              : targets
                  .map(
                    (target) =>
                      `${target.label}${target.installed ? "" : ` ${t.mcps.notInstalledMark}`}`,
                  )
                  .join(" · ")}
          </span>
        </div>
      </div>

      {inspecting && (
        <McpDetailDrawer
          server={servers.find((server) => server.id === inspecting.id) ?? inspecting}
          scopeLabel={scopeLabel}
          onClose={() => setInspecting(null)}
          onEdit={(item) => {
            setInspecting(null);
            setEditing(item);
            setFormError(null);
            setFormOpen(true);
          }}
        />
      )}

      {formOpen && (
        <McpFormModal
          server={editing}
          existingIds={servers.map((server) => server.id)}
          saving={saving}
          error={formError}
          onClose={() => {
            setFormOpen(false);
            setEditing(null);
          }}
          onSave={(server) => void save(server)}
        />
      )}
    </div>
  );
}
