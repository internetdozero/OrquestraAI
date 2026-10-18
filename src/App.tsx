import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { CheckCircle2 } from "lucide-react";
import { AgentsView } from "./components/AgentsView";
import { BootScreen } from "./components/BootScreen";
import { BridgeModal } from "./components/BridgeModal";
import { Header } from "./components/Header";
import { IdeasView } from "./components/ideas/IdeasView";
import { JanitorModal } from "./components/JanitorModal";
import { McpView } from "./components/McpView";
import { PresetBar } from "./components/PresetBar";
import { SettingsModal } from "./components/settings/SettingsModal";
import { Sidebar } from "./components/Sidebar";
import { SessionsView } from "./components/sessions/SessionsView";
import { SkillsView } from "./components/SkillsView";
import { StatusFooter } from "./components/StatusFooter";
import { useHarnessBridge } from "./hooks/useHarnessBridge";
import { useHarnessSummaries } from "./hooks/useHarnessSummaries";
import { useJanitor } from "./hooks/useJanitor";
import { useI18n } from "./hooks/useI18n";
import { useAppSettings } from "./hooks/useAppSettings";
import { usePresets } from "./hooks/usePresets";
import { useToast } from "./hooks/useToast";
import { ensureMcpCatalog, refreshMcpCatalog } from "./lib/mcpCache";
import { dropSkillsCache } from "./lib/skillsCache";
import { setMinimizeToTray } from "./lib/tauri";
import { preloadBackground, preloadEssentials } from "./lib/preloadResources";
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
  const { t } = useI18n();
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
  const [skillsReady, setSkillsReady] = useState(false);
  const [janitorOpen, setJanitorOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const { toast, showToast } = useToast();
  const prefs = useAppSettings();
  const clearError = useCallback(() => setError(null), []);
  const janitor = useJanitor({
    isTauri,
    notifyOnPurge: prefs.settings.notifyOnPurge,
    active: janitorOpen,
    onToast: showToast,
    onError: setError,
  });
  const summaries = useHarnessSummaries(isTauri, setError, reloadToken);
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
      // Belt-and-braces with usePresets: drop the skills grid cache and
      // force the MCP catalog to re-read so the new preset paints instantly.
      dropSkillsCache();
      void refreshMcpCatalog().catch(() => undefined);
      setReloadToken((token) => token + 1);
      void summaries.refresh();
    },
  });

  // Push the window-close behavior to the Rust core on startup and every
  // toggle, so `minimizeToTray` actually hides to the tray instead of
  // exiting. Failures are silent: the backend default already hides.
  useEffect(() => {
    if (!isTauri) return;
    void setMinimizeToTray(prefs.settings.minimizeToTray).catch(() => undefined);
  }, [isTauri, prefs.settings.minimizeToTray]);

  // Edge-triggered RAM alert against the operator's own threshold (the
  // health poll runs every 5s; this toasts once per crossing, not per poll).
  const ramAlertedRef = useRef(false);
  useEffect(() => {
    const percent = janitor.health?.ram.usedPercent;
    if (percent == null) return;
    if (percent > prefs.settings.ramAlertThreshold) {
      if (!ramAlertedRef.current) {
        ramAlertedRef.current = true;
        showToast(t.janitor.ramAlert(Math.round(percent), prefs.settings.ramAlertThreshold));
      }
    } else {
      ramAlertedRef.current = false;
    }
  }, [janitor.health, prefs.settings.ramAlertThreshold, showToast, t]);

  // Native desktop feel: suppress the browser context menu everywhere except
  // editable fields, where copy/paste must keep working.
  useEffect(() => {
    const onContextMenu = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      const editable =
        target !== null &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable);
      if (!editable) event.preventDefault();
    };

    document.addEventListener("contextmenu", onContextMenu);
    return () => document.removeEventListener("contextmenu", onContextMenu);
  }, []);

  useEffect(() => {
    if (!isTauri) return;

    let cancelled = false;
    preloadEssentials()
      .catch(() => undefined)
      .finally(() => {
        if (!cancelled) setSkillsReady(true);
      });

    return () => {
      cancelled = true;
    };
  }, [isTauri]);

  useEffect(() => {
    if (!isTauri || !skillsReady) return;

    let cancelled = false;
    void preloadBackground(() => cancelled);
    return () => {
      cancelled = true;
    };
  }, [isTauri, skillsReady]);

  useEffect(() => {
    if (!isTauri || !skillsReady) return;

    let cancelled = false;
    const read = reloadToken === 0 ? ensureMcpCatalog() : refreshMcpCatalog();
    read
      .then((catalog) => {
        if (cancelled) return;

        setMcpCount(
          catalog.servers.filter(
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
  }, [isTauri, reloadToken, skillsReady]);

  const essentialsReady = !isTauri || (skillsReady && !presets.loading);

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

  if (!essentialsReady) return <BootScreen />;

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
        onRefresh={() => {
          setReloadToken((token) => token + 1);
          void summaries.refresh();
          showToast(t.common.refreshSkills);
        }}
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
          onUpdate={presets.update}
          onReset={presets.reset}
          onDelete={(preset) => void presets.remove(preset)}
        />
      )}

      <div className="relative z-10 flex min-h-0 flex-1 overflow-hidden">
        <Sidebar
          view={view}
          onSelect={selectView}
          skillStats={{ active: skillStats.active, total: totalSkills }}
          mcpCount={mcpCount}
          onOpenSettings={() => setSettingsOpen(true)}
        />

        <main
          className={cn(
            "relative flex-1",
            view === "agents"
              ? "overflow-hidden px-4 py-4 sm:px-5"
              : view === "mcps"
                ? "overflow-hidden"
                : "scroll-pane overflow-y-auto px-4 py-5 sm:px-6",
            // sessions shares the scroll-pane layout with skills

          )}
        >
          {view === "ideas" ? (
            <IdeasView isTauri={isTauri} onToast={showToast} onError={setError} />
          ) : view === "agents" ? (
            <AgentsView
              isTauri={isTauri}
              error={error}
              onToast={showToast}
              onError={setError}
              onClear={clearError}
            />
          ) : view === "sessions" ? (
            <SessionsView
              isTauri={isTauri}
              notifyOnPurge={prefs.settings.notifyOnPurge}
              onToast={showToast}
              onError={setError}
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
              onToast={showToast}
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
        janitorHealth={janitor.health}
        ramAlertThreshold={prefs.settings.ramAlertThreshold}
        onOpenJanitor={() => setJanitorOpen(true)}
      />

      {janitorOpen && (
        <JanitorModal
          health={janitor.health}
          ramWarning={prefs.settings.ramAlertThreshold}
          loading={janitor.loading}
          sweeping={janitor.sweeping}
          onClose={() => setJanitorOpen(false)}
          onRefresh={() => void janitor.refresh()}
          onSweep={() => void janitor.sweep()}
        />
      )}

      {settingsOpen && (
        <SettingsModal
          prefs={prefs}
          status={status}
          isTauri={isTauri}
          onClose={() => setSettingsOpen(false)}
          onToast={showToast}
          onError={setError}
          onOpenSessions={() => {
            setSettingsOpen(false);
            selectView("sessions");
          }}
        />
      )}

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
