import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { KeyRound } from "lucide-react";
import { devMockMode } from "../../lib/fixtures.ts";
import {
  listVaultPresets,
  vaultDeleteService,
  vaultListServices,
  vaultPurgeMigrationBackups,
  vaultRemoveKey,
  vaultSaveService,
  vaultSetPrimary,
  vaultTestConnection,
  type VaultPreset,
  type VaultService,
  type VaultTestResult,
} from "../../lib/vault.ts";
import { useI18n } from "../../hooks/useI18n.ts";
import { AddCredentialModal } from "../vault/AddCredentialModal.tsx";
import { MigrationBackupPurgeModal } from "./MigrationBackupPurgeModal.tsx";
import {
  CredentialsHeader,
  type CredentialStatusFilter,
  type CredentialViewMode,
} from "./CredentialsHeader.tsx";
import { ServiceCard } from "./ServiceCard.tsx";
import { ServiceListRow } from "./ServiceListRow.tsx";
import { StatePanel } from "../ui.tsx";

interface CredentialsViewProps {
  isTauri: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
  /** Shell refresh button bumps this to reload services. */
  reloadToken?: number;
}

export function CredentialsView({ isTauri, onToast, onError, reloadToken = 0 }: CredentialsViewProps) {
  const [services, setServices] = useState<VaultService[]>([]);
  const [presets, setPresets] = useState<VaultPreset[]>([]);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<CredentialStatusFilter>("All");
  const [typeFilter, setTypeFilter] = useState("All");
  const [view, setView] = useState<CredentialViewMode>(() =>
    localStorage.getItem("credentials-view") === "list" ? "list" : "grid",
  );
  const [modalOpen, setModalOpen] = useState(false);
  const [purgeOpen, setPurgeOpen] = useState(false);
  const [purging, setPurging] = useState(false);
  // Browser preview (vite dev without the Tauri core) renders fixture data
  // so the dashboard stays reviewable outside the desktop app, like McpView.
  const [mockMode] = useState(devMockMode);
  const { t } = useI18n();
  const s = t.credentials;

  const refresh = useCallback(async () => {
    setServices(await vaultListServices());
  }, []);

  // The shell asks for a rescan by bumping the token; skip the mount value.
  const lastReloadToken = useRef(reloadToken);
  useEffect(() => {
    if (reloadToken === lastReloadToken.current) return;
    lastReloadToken.current = reloadToken;
    void refresh();
  }, [reloadToken, refresh]);

  useEffect(() => {
    if (!isTauri && !mockMode) {
      setLoading(false);
      return;
    }
    let cancelled = false;
    Promise.all([listVaultPresets(), vaultListServices()])
      .then(([presetList, serviceList]) => {
        if (cancelled) return;
        setPresets(presetList);
        setServices(serviceList);
      })
      .catch((err: unknown) => {
        if (!cancelled) onError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isTauri, mockMode, onError]);

  const presetLabels = useMemo(() => {
    const labels = new Map<string, string>();
    for (const preset of presets) labels.set(preset.id, preset.label);
    return labels;
  }, [presets]);

  const metrics = useMemo(() => {
    let keys = 0;
    let healthy = 0;
    for (const service of services) {
      keys += service.totalKeys;
      if (service.health === "Healthy") healthy += 1;
    }
    return {
      totalServices: services.length,
      totalKeys: keys,
      healthyServices: healthy,
      degradedServices: services.length - healthy,
    };
  }, [services]);

  useEffect(() => {
    localStorage.setItem("credentials-view", view);
  }, [view]);

  const typeOptions = useMemo(() => {
    const seen = new Map<string, string>();
    for (const service of services) {
      if (!seen.has(service.preset)) {
        seen.set(service.preset, presetLabels.get(service.preset) ?? service.preset);
      }
    }
    return [
      { id: "All", label: s.typeAll },
      ...[...seen.entries()].map(([id, label]) => ({ id, label })),
    ];
  }, [services, presetLabels, s.typeAll]);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return services.filter((service) => {
      if (statusFilter !== "All" && service.health !== statusFilter) return false;
      if (typeFilter !== "All" && service.preset !== typeFilter) return false;
      if (needle.length === 0) return true;
      return (
        service.label.toLowerCase().includes(needle) ||
        service.id.toLowerCase().includes(needle) ||
        service.preset.toLowerCase().includes(needle)
      );
    });
  }, [services, query, statusFilter, typeFilter]);

  async function purgeBackups(): Promise<void> {
    setPurging(true);
    try {
      const report = await vaultPurgeMigrationBackups();
      setPurgeOpen(false);
      onToast(s.toastBackupsPurged(report.vaultPurged, report.catalogPurged));
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setPurging(false);
    }
  }

  async function mutate(action: () => Promise<unknown>, done: string): Promise<void> {
    try {
      await action();
      await refresh();
      onToast(done);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  }

  /** Shared handlers so grid cards and list rows stay in sync. */
  function handlersFor(service: VaultService) {
    return {
      service,
      presetId: service.preset,
      presetLabel: presetLabels.get(service.preset) ?? service.preset,
      presets,
      onPromote: (index: number) =>
        mutate(() => vaultSetPrimary(service.id, index), s.toastPrimary(service.label)),
      onRemoveKey: (index: number) =>
        mutate(() => vaultRemoveKey(service.id, index), s.toastRemoved(service.label)),
      onAddKeys: (keys: string[], keyTags: (string | null)[]) =>
        mutate(
          () =>
            vaultSaveService({
              id: service.id,
              label: service.label,
              preset: service.preset,
              description: service.description,
              keys: [...service.maskedKeys, ...keys],
              keyTags: [...(service.keyTags ?? service.maskedKeys.map(() => null)), ...keyTags],
            }),
          s.toastExtended(service.label),
        ),
      onDelete: () => mutate(() => vaultDeleteService(service.id), s.toastDeleted(service.label)),
      onSaveTag: (index: number, tag: string | null) =>
        mutate(
          () =>
            vaultSaveService({
              id: service.id,
              label: service.label,
              preset: service.preset,
              description: service.description,
              keys: [...service.maskedKeys],
              keyTags: service.maskedKeys.map((_, position) =>
                position === index ? tag : (service.keyTags?.[position] ?? null),
              ),
            }),
          s.toastTag(index, service.label),
        ),
      onReplaceKey: (index: number, secret: string) =>
        mutate(
          () =>
            vaultSaveService({
              id: service.id,
              label: service.label,
              preset: service.preset,
              description: service.description,
              keys: service.maskedKeys.map((masked, position) =>
                position === index ? secret : masked,
              ),
              keyTags: [...(service.keyTags ?? service.maskedKeys.map(() => null))],
            }),
          s.toastRotated(index, service.label),
        ),
      onSaveService: (input: {
        label: string;
        preset: string;
        description: string;
        meta: Record<string, string>;
        customHeader?: string;
        customPrefix?: string;
        customEnv?: string;
      }) =>
        mutate(
          () =>
            vaultSaveService({
              id: service.id,
              label: input.label,
              preset: input.preset,
              description: input.description,
              keys: [...service.maskedKeys],
              keyTags: [...(service.keyTags ?? service.maskedKeys.map(() => null))],
              customHeader: input.customHeader,
              customPrefix: input.customPrefix,
              customEnv: input.customEnv,
              meta: input.meta,
            }),
          s.toastUpdated(service.label),
        ),
      onTest: async (index: number): Promise<VaultTestResult> => {
        const result = await vaultTestConnection(service.id, index);
        await refresh();
        return result;
      },
    };
  }

  if (!isTauri && !mockMode) {
    return (
      <StatePanel icon={KeyRound} title={s.needDesktopTitle} description={s.needDesktopBody} />
    );
  }

  if (loading) {
    return <StatePanel icon={KeyRound} title={s.loadingTitle} description={s.loadingBody} />;
  }

  return (
    <div className="mx-auto w-full max-w-6xl pb-8">
      <CredentialsHeader
        totalServices={metrics.totalServices}
        totalKeys={metrics.totalKeys}
        healthyServices={metrics.healthyServices}
        degradedServices={metrics.degradedServices}
        query={query}
        onQueryChange={setQuery}
        statusFilter={statusFilter}
        onStatusFilterChange={setStatusFilter}
        typeOptions={typeOptions}
        typeFilter={typeFilter}
        onTypeFilterChange={setTypeFilter}
        view={view}
        onViewChange={setView}
        onAdd={() => setModalOpen(true)}
        onPurgeBackups={isTauri && !mockMode ? () => setPurgeOpen(true) : undefined}
      />

      {visible.length === 0 ? (
        <div className="mt-6">
          {services.length === 0 ? (
            <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed border-white/10 bg-zinc-900/20 px-6 py-16 text-center">
              <div className="grid size-12 place-items-center rounded-full border border-emerald-500/30 bg-emerald-500/10 shadow-[0_0_32px_-6px_rgba(16,185,129,0.7)]">
                <KeyRound className="size-5 text-emerald-300" />
              </div>
              <p className="text-sm font-semibold text-zinc-200">{s.emptyTitle}</p>
              <p className="max-w-sm text-[12px] leading-relaxed text-zinc-500">{s.emptyBody}</p>
              <button
                type="button"
                onClick={() => setModalOpen(true)}
                className="mt-1 flex items-center gap-1.5 rounded-lg border border-emerald-500/30 bg-emerald-500/15 px-3 py-2 text-[11.5px] font-medium text-emerald-200 shadow-[0_0_24px_-8px_rgba(16,185,129,0.6)] transition hover:bg-emerald-500/25"
              >
                {s.registerFirst}
              </button>
            </div>
          ) : (
            <StatePanel icon={KeyRound} title={s.noMatchesTitle} description={s.noMatchesBody} />
          )}
        </div>
      ) : view === "grid" ? (
        <div className="mt-4 grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
          {visible.map((service) => (
            <ServiceCard key={service.id} {...handlersFor(service)} />
          ))}
        </div>
      ) : (
        <div className="mt-4 space-y-2">
          {visible.map((service) => (
            <ServiceListRow key={service.id} {...handlersFor(service)} />
          ))}
        </div>
      )}

      <AddCredentialModal
        open={modalOpen}
        presets={presets.map((preset) => ({
          ...preset,
          description: s.presetDescriptions[preset.id] ?? preset.description,
        }))}
        onClose={() => setModalOpen(false)}
        onSave={async (input) => {
          await vaultSaveService({
            ...input,
            description: input.label,
          });
          await refresh();
          onToast(s.toastSaved(input.id));
        }}
      />
      <MigrationBackupPurgeModal
        open={purgeOpen}
        purging={purging}
        onCancel={() => setPurgeOpen(false)}
        onConfirm={() => void purgeBackups()}
      />
    </div>
  );
}
