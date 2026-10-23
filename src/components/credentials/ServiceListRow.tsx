import { useState } from "react";
import { Crown, LayoutGrid, Loader2, PlugZap, Trash2 } from "lucide-react";
import type { VaultPreset, VaultService, VaultTestResult } from "../../lib/vault.ts";
import { cn } from "../../lib/utils.ts";
import { useI18n } from "../../hooks/useI18n.ts";
import { BrandIcon } from "../vault/BrandIcons.tsx";
import { ServiceDetailModal } from "./ServiceDetailModal.tsx";
import { ServiceEnabledToggle } from "./ServiceEnabledToggle.tsx";
import { StatusDot } from "./StatusDot.tsx";
import type { ServiceEditInput } from "./ServiceEditForm.tsx";

interface ServiceListRowProps {
  service: VaultService;
  presetId: string;
  presetLabel: string;
  presets: VaultPreset[];
  onPromote: (index: number) => Promise<void>;
  onRemoveKey: (index: number) => Promise<void>;
  onAddKeys: (keys: string[], keyTags: (string | null)[]) => Promise<void>;
  onDelete: () => Promise<void>;
  onTest: (index: number) => Promise<VaultTestResult>;
  onSaveTag: (index: number, tag: string | null) => Promise<void>;
  onReplaceKey: (index: number, secret: string) => Promise<void>;
  onSaveService: (input: ServiceEditInput) => Promise<void>;
  onToggleEnabled: () => Promise<void>;
}

/** Single-line summary for list view; full management lives in the modal. */
export function ServiceListRow({
  service,
  presetId,
  presetLabel,
  presets,
  onPromote,
  onRemoveKey,
  onAddKeys,
  onDelete,
  onTest,
  onSaveTag,
  onReplaceKey,
  onSaveService,
  onToggleEnabled,
}: ServiceListRowProps) {
  const [detailOpen, setDetailOpen] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [busy, setBusy] = useState(false);
  const [testing, setTesting] = useState(false);
  const [toggling, setToggling] = useState(false);
  const { t } = useI18n();
  const s = t.credentials;

  const primaryMasked = service.maskedKeys[service.activeKeyIndex] ?? service.maskedKeys[0] ?? "";
  const primaryTag = service.keyTags?.[service.activeKeyIndex] ?? null;

  async function handleTestPrimary(): Promise<void> {
    setTesting(true);
    try {
      await onTest(service.activeKeyIndex);
    } finally {
      setTesting(false);
    }
  }

  async function handleToggleEnabled(): Promise<void> {
    setToggling(true);
    try {
      await onToggleEnabled();
    } finally {
      setToggling(false);
    }
  }

  async function handleDelete(): Promise<void> {
    if (!confirmingDelete) {
      setConfirmingDelete(true);
      return;
    }
    setBusy(true);
    try {
      await onDelete();
    } finally {
      setBusy(false);
      setConfirmingDelete(false);
    }
  }

  return (
    <>
      <div
        className={cn(
          "flex items-center gap-2.5 rounded-xl border border-white/[0.07] bg-zinc-900/60 px-3 py-2.5",
          !service.enabled && "opacity-60",
        )}
      >
        <ServiceEnabledToggle
          enabled={service.enabled}
          busy={toggling || busy}
          label={service.label}
          onToggle={() => void handleToggleEnabled()}
        />
        <span className="shrink-0 overflow-hidden rounded-lg border border-white/[0.07]">
          <BrandIcon presetId={presetId} size={32} />
        </span>
        <div className="min-w-0 w-44 shrink-0">
          <h3 className="truncate text-[13px] font-semibold text-zinc-100">{service.label}</h3>
          <p className="truncate font-mono text-[10px] text-zinc-500">{service.id}</p>
        </div>
        <span className="hidden shrink-0 rounded bg-white/[0.06] px-1.5 py-0.5 text-[10.5px] font-medium text-zinc-300 sm:inline">
          {presetLabel}
        </span>
        <span className="hidden shrink-0 text-[10.5px] text-zinc-500 md:inline">
          {s.keysCount(service.totalKeys)}
        </span>
        <code className="hidden min-w-0 flex-1 truncate font-mono text-xs text-zinc-400 lg:inline">
          <Crown className="mr-1 inline size-3 text-emerald-300" />
          {primaryMasked}
        </code>
        {primaryTag !== null && primaryTag.trim() !== "" && (
          <span className="hidden shrink-0 rounded-full border border-violet-500/40 bg-violet-500/15 px-1.5 py-px font-mono text-[10px] font-medium text-violet-200 xl:inline">
            {primaryTag.trim()}
          </span>
        )}
        <StatusDot health={service.health} label={s.healthName(service.health)} />
        <span className="flex-1 sm:hidden" />
        <div className="flex shrink-0 items-center gap-1">
          <button
            type="button"
            onClick={() => setDetailOpen(true)}
            title={s.detailsAria}
            className="flex items-center gap-1 rounded bg-white/[0.06] px-2 py-1 text-[11.5px] font-medium text-zinc-200 transition hover:bg-white/[0.1] hover:text-white"
          >
            <LayoutGrid className="size-3.5" />
            <span className="hidden sm:inline">{s.details}</span>
          </button>
          <button
            type="button"
            onClick={() => void handleTestPrimary()}
            disabled={testing || busy}
            title={s.testPrimaryTitle}
            className="rounded p-1.5 text-zinc-400 transition hover:bg-white/[0.05] hover:text-zinc-100 disabled:opacity-50"
          >
            {testing ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <PlugZap className="size-3.5" />
            )}
          </button>
          <button
            type="button"
            onClick={() => void handleDelete()}
            onBlur={() => setConfirmingDelete(false)}
            disabled={busy}
            title={confirmingDelete ? s.confirmDeleteTitle : s.deleteTitle}
            aria-label={confirmingDelete ? s.confirmDelete : s.deleteTitle}
            className={cn(
              "rounded p-1.5 transition disabled:opacity-50",
              confirmingDelete
                ? "bg-red-500/15 text-red-200"
                : "text-zinc-500 hover:bg-red-500/10 hover:text-red-300",
            )}
          >
            <Trash2 className="size-3.5" />
          </button>
        </div>
      </div>

      <ServiceDetailModal
        open={detailOpen}
        service={service}
        presetId={presetId}
        presetLabel={presetLabel}
        presets={presets}
        onClose={() => setDetailOpen(false)}
        onPromote={onPromote}
        onRemoveKey={onRemoveKey}
        onAddKeys={onAddKeys}
        onDelete={onDelete}
        onTest={onTest}
        onSaveTag={onSaveTag}
        onReplaceKey={onReplaceKey}
        onSaveService={onSaveService}
      />
    </>
  );
}
