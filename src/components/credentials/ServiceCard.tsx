import { useState } from "react";
import { Crown, LayoutGrid, Loader2, PlugZap, Trash2 } from "lucide-react";
import type { VaultPreset, VaultService, VaultTestResult } from "../../lib/vault.ts";
import type { ServiceEditInput } from "./ServiceEditForm.tsx";
import { cn } from "../../lib/utils.ts";
import { useI18n } from "../../hooks/useI18n.ts";
import { BrandIcon } from "../vault/BrandIcons.tsx";
import { ServiceDetailModal } from "./ServiceDetailModal.tsx";
import { StatusDot } from "./StatusDot.tsx";

interface ServiceCardProps {
  service: VaultService;
  presetId: string;
  presetLabel: string;
  onPromote: (index: number) => Promise<void>;
  onRemoveKey: (index: number) => Promise<void>;
  onAddKeys: (keys: string[], keyTags: (string | null)[]) => Promise<void>;
  onDelete: () => Promise<void>;
  onTest: (index: number) => Promise<VaultTestResult>;
  onSaveTag: (index: number, tag: string | null) => Promise<void>;
  onReplaceKey: (index: number, secret: string) => Promise<void>;
  onSaveService: (input: ServiceEditInput) => Promise<void>;
  presets: VaultPreset[];
}

/** Compact uniform summary: header, preset, primary key preview. Everything
 * else lives in the detail modal so every card renders at the same height. */
export function ServiceCard({
  service,
  presetId,
  presetLabel,
  onPromote,
  onRemoveKey,
  onAddKeys,
  onDelete,
  onTest,
  onSaveTag,
  onReplaceKey,
  onSaveService,
  presets,
}: ServiceCardProps) {
  const [detailOpen, setDetailOpen] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [busy, setBusy] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testNote, setTestNote] = useState<string | null>(null);
  const { t } = useI18n();
  const s = t.credentials;

  const primaryMasked = service.maskedKeys[service.activeKeyIndex] ?? service.maskedKeys[0] ?? "";
  const primaryTag = service.keyTags?.[service.activeKeyIndex] ?? null;

  async function handleTestPrimary(): Promise<void> {
    setTesting(true);
    try {
      const result = await onTest(service.activeKeyIndex);
      setTestNote(result.success ? `✓ ${result.message}` : `✗ ${result.message}`);
    } catch (err) {
      setTestNote(`✗ ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setTesting(false);
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
      <div className="flex h-full flex-col rounded-xl border border-white/[0.07] bg-zinc-900/60 p-3.5">
        <div className="flex items-start gap-2.5">
          <span className="shrink-0 overflow-hidden rounded-lg border border-white/[0.07]">
            <BrandIcon presetId={presetId} size={36} />
          </span>
          <div className="min-w-0 flex-1">
            <h3 className="truncate text-sm font-semibold text-zinc-100">
              {service.label}
            </h3>
            <p className="mt-0.5 truncate font-mono text-[10.5px] text-zinc-500">
              {service.id}
            </p>
          </div>
          <StatusDot
            health={service.health}
            label={s.healthName(service.health)}
            className="mt-1"
          />
        </div>

      <div className="mt-2 flex items-center gap-1.5">
        <span className="rounded bg-white/[0.06] px-1.5 py-0.5 text-[10.5px] font-medium text-zinc-300">
          {presetLabel}
        </span>
        <span className="text-[10.5px] text-zinc-500">
          {s.keysCount(service.totalKeys)} · {s.primaryShort(service.activeKeyIndex)}
        </span>
      </div>

        <div className="mt-2.5 flex items-center gap-2 rounded-md border border-emerald-500/40 bg-emerald-500/[0.06] px-2 py-1.5 text-xs">
          <Crown className="size-3.5 shrink-0 text-emerald-300" />
          <code className="min-w-0 flex-1 truncate font-mono text-zinc-200">
            {primaryMasked}
          </code>
          {primaryTag !== null && primaryTag.trim() !== "" && (
            <span className="shrink-0 rounded-full border border-violet-500/40 bg-violet-500/15 px-1.5 py-px font-mono text-[10px] font-medium text-violet-200">
              {primaryTag.trim()}
            </span>
          )}
        </div>

        {testNote !== null && (
          <p className="mt-1.5 truncate text-[11px] text-zinc-400" title={testNote}>
            {testNote}
          </p>
        )}

        <div className="mt-auto flex items-center gap-1.5 pt-2.5">
          <button
            type="button"
            onClick={() => setDetailOpen(true)}
            className="flex items-center gap-1 rounded bg-white/[0.06] px-2 py-1 text-[11.5px] font-medium text-zinc-200 transition hover:bg-white/[0.1] hover:text-white"
          >
            <LayoutGrid className="size-3.5" /> {s.details}
          </button>
          <button
            type="button"
            onClick={() => void handleTestPrimary()}
            disabled={testing || busy}
            title={s.testPrimaryTitle}
            className="flex items-center gap-1 rounded px-1.5 py-1 text-[11.5px] text-zinc-400 transition hover:bg-white/[0.05] hover:text-zinc-100 disabled:opacity-50"
          >
            {testing ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <PlugZap className="size-3.5" />
            )}
            {s.test}
          </button>
          <span className="flex-1" />
          <button
            type="button"
            onClick={() => void handleDelete()}
            onBlur={() => setConfirmingDelete(false)}
            disabled={busy}
            className={cn(
              "flex items-center gap-1 rounded px-1.5 py-1 text-[11.5px] transition disabled:opacity-50",
              confirmingDelete
                ? "bg-red-500/15 text-red-200"
                : "text-zinc-500 hover:bg-red-500/10 hover:text-red-300",
            )}
          >
            <Trash2 className="size-3.5" />
            {confirmingDelete ? s.confirmShort : s.delete}
          </button>
        </div>
      </div>

      <ServiceDetailModal
        open={detailOpen}
        service={service}
        presetId={presetId}
        presetLabel={presetLabel}
        onClose={() => setDetailOpen(false)}
        onPromote={onPromote}
        onRemoveKey={onRemoveKey}
        onAddKeys={onAddKeys}
        onDelete={onDelete}
        onTest={onTest}
        onSaveTag={onSaveTag}
        onReplaceKey={onReplaceKey}
        onSaveService={onSaveService}
        presets={presets}
      />
    </>
  );
}
