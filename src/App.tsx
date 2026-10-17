import { useCallback, useEffect, useMemo, useState } from "react";
import { CheckCircle2 } from "lucide-react";
import { AgentsView } from "./components/AgentsView";
import { BridgeModal } from "./components/BridgeModal";
import { Header } from "./components/Header";
import { McpView } from "./components/McpView";
import { PresetBar } from "./components/PresetBar";
import { Sidebar } from "./components/Sidebar";
import { SkillsView } from "./components/SkillsView";
import { StatusFooter } from "./components/StatusFooter";
import { useHarnessBridge } from "./hooks/useHarnessBridge";
import { useHarnessSummaries } from "./hooks/useHarnessSummaries";
import { usePresets } from "./hooks/usePresets";
import { useToast } from "./hooks/useToast";
import { listMcpServers } from "./lib/mcps";
import { useCoreStatus } from "./lib/tauri";
import { cn } from "./lib/utils";
import type { ActiveView } from "./lib/views";

/**
 * Application shell: owns the active platform view and the data the header
 * mirrors (counters, search, refresh). Harness scopes are not routes — the
 * skills view keeps its own scope tabs, so switching harnesses never remounts
 * the shell.
 */
export default function App() {
  const { status, isTauri } = useCoreStatus();
  const [view, setView] = useState<ActiveView>("skills");
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [skillStats, setSkillStats] = useState({ active: 0, total: 0 });
  const [skillsBusy, setSkillsBusy] = useState(true);
  // Bumped whenever something outside the grid invalidated the skill list.
  const [reloadToken, setReloadToken] = useState(0);
  // Count of MCP servers enabled somewhere, for the sidebar badge only; the MCP
  // view keeps its own copy and reports changes back through `onActiveCountChange`.
  const [mcpCount, setMcpCount] = useState(0);

  const { toast, showToast } = useToast();
  const clearError = useCallback(() => setError(null), []);
  const summaries = useHarnessSummaries(isTauri, setError);
  const bridge =  useHarnessBridge({
    isTauri,
    onToast: showToast,
    onError: setError,
    onDeployed: () => {
      setReloadToken((token) => token + 1);
      void summaries.refresh();
    },
  });

  // A preset rewrites the skill projections and the MCP catalog, so a single
  // apply invalidates the grid, the sidebar counters and the MCP count.
  const presets = usePresets({
    isTauri,
    onToast: showToast,
    onError: setError,
    onApplied: () => {
      setReloadToken((token) => token + 1);
      void summaries.refresh();
    },
  });

  useEffect(() => {
    if (!isTauri) return;

    let cancelled = false;
    listMcpServers()
      .then((catalog) => {
        if (cancelled) return;

        setMcpCount(
          catalog.filter(
            (server) =>
              server.globalEnabled || server.enabledHarnesses.length > 0,
          ).length,
        );
      })
      .catch(() => {
        /* The badge is cosmetic; the MCP view surfaces real read errors. */
      });

    return () => {
      cancelled = true;
    };
  }, [isTauri, reloadToken]);

  // Harnesses the bridge could not find are dimmed in the skills scope tabs.
  const installedHarnessIds = useMemo(
    () =>
      bridge.statuses.length === 0
        ? null
        : new Set(
            bridge.statuses
              .filter((harness) => harness.installed)
              .map((harness) => harness.id),
          ),
    [bridge.statuses],
  );

  const totalSkills = summaries.total || skillStats.total;

  const selectView = (next: ActiveView) => {
    if (next === view) return;

    setView(next);
    setQuery("");
    clearError();
  };

  return (
    <div className="relative flex h-full flex-col overflow-hidden bg-[#09090b] text-zinc-100">
      {/* Ambient background wash */}
      <div className="pointer-events-none absolute inset-0 overflow-hidden">
        <div className="absolute -top-40 left-1/4 size-[32rem] rounded-full bg-emerald-500/[0.06] blur-[120px]" />
        <div className="absolute top-1/3 -right-32 size-[26rem] rounded-full bg-cyan-500/[0.05] blur-[120px]" />
      </div>

      <Header
        mode={view}
        query={query}
        onQueryChange={setQuery}
        activeCount={skillStats.active}
        totalCount={skillStats.total}
        onRefresh={() => setReloadToken((token) => token + 1)}
        refreshing={skillsBusy}
      />

      {isTauri && (
        <PresetBar
          presets={presets.presets}
          loading={presets.loading}
          busyId={presets.busyId}
          saving={presets.saving}
          onApply={(preset) => void presets.apply(preset)}
          onSave={presets.save}
          onDelete={(preset) => void presets.remove(preset)}
        />
      )}

      <div className="relative z-10 flex min-h-0 flex-1 overflow-hidden">
        <Sidebar
          view={view}
          onSelect={selectView}
          skillStats={{ active: skillStats.active, total: totalSkills }}
          mcpCount={mcpCount}
        />

        <main
          className={cn(
            "relative flex-1",
            view === "agents"
              ? "overflow-hidden px-4 py-4 sm:px-5"
              : view === "mcps"
                ? "overflow-hidden"
                : "scroll-pane overflow-y-auto px-4 py-5 sm:px-6",
          )}
        >
          {view === "agents" ? (
            <AgentsView
              isTauri={isTauri}
              error={error}
              onToast={showToast}
              onError={setError}
              onClear={clearError}
            />
          ) : view === "mcps" ? (
            <McpView
              isTauri={isTauri}
              onToast={showToast}
              onActiveCountChange={setMcpCount}
              reloadToken={reloadToken}
            />
          ) : (
            <SkillsView
              isTauri={isTauri}
              harnessCounts={summaries.counts}
              harnessTotal={summaries.total}
              installedHarnessIds={installedHarnessIds}
              query={query}
              onQueryChange={setQuery}
              reloadToken={reloadToken}
              onStatsChange={setSkillStats}
              onSummaryStale={() => void summaries.refresh()}
              onBusyChange={setSkillsBusy}
              externalError={error}
              onDismissExternalError={clearError}
            />
          )}
        </main>
      </div>

      <StatusFooter
        status={status}
        isTauri={isTauri}
        bridgeStatuses={bridge.statuses}
        bridgeDeploying={bridge.deploying}
        onManageBridge={() => bridge.setOpen(true)}
      />

      {bridge.open && (
        <BridgeModal
          statuses={bridge.statuses}
          deploying={bridge.deploying}
          onClose={() => bridge.setOpen(false)}
          onConfirm={() => void bridge.deploy()}
        />
      )}

      {toast && (
        <div
          role="status"
          className="pointer-events-none fixed right-5 bottom-12 z-50 flex items-center gap-2 rounded-lg border border-emerald-500/25 bg-emerald-500/10 px-3 py-2 text-[11.5px] font-medium text-emerald-200 shadow-[0_8px_30px_-10px_rgba(0,0,0,0.8)] backdrop-blur"
        >
          <CheckCircle2 className="size-3.5" />
          {toast}
        </div>
      )}
    </div>
  );
}
