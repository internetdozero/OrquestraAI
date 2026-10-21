import { useState } from "react";
import { Loader2, Pencil, PlugZap, Plus, Trash2, X } from "lucide-react";
import type { VaultPreset, VaultService, VaultTestResult } from "../../lib/vault.ts";
import { cn } from "../../lib/utils.ts";
import { useI18n } from "../../hooks/useI18n.ts";
import { BrandIcon } from "../vault/BrandIcons.tsx";
import { ServiceEditForm, type ServiceEditInput } from "./ServiceEditForm.tsx";
import { ServiceKeyRow } from "./ServiceKeyRow.tsx";
import { StatusDot } from "./StatusDot.tsx";

interface ServiceDetailModalProps {
  open: boolean;
  service: VaultService;
  presetId: string;
  presetLabel: string;
  onClose: () => void;
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

/** Full pool management for one service: every key, tags, meta and actions. */
export function ServiceDetailModal({
  open,
  service,
  presetId,
  presetLabel,
  onClose,
  onPromote,
  onRemoveKey,
  onAddKeys,
  onDelete,
  onTest,
  onSaveTag,
  onReplaceKey,
  onSaveService,
  presets,
}: ServiceDetailModalProps) {
  const [adding, setAdding] = useState(false);
  const [newKey, setNewKey] = useState("");
  const [newTag, setNewTag] = useState("");
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [busy, setBusy] = useState(false);
  const [testingPrimary, setTestingPrimary] = useState(false);
  const [testingAll, setTestingAll] = useState(false);
  const [editing, setEditing] = useState(false);
  const { t } = useI18n();
  const s = t.credentials;
  const [testNote, setTestNote] = useState<string | null>(null);

  if (!open) return null;

  async function handleAdd(): Promise<void> {
    const keys = newKey
      .split("\n")
      .map((key) => key.trim())
      .filter(Boolean);
    if (keys.length === 0) return;
    const tag = newTag.trim() || null;
    setBusy(true);
    try {
      await onAddKeys(
        keys,
        keys.map(() => tag),
      );
      setNewKey("");
      setNewTag("");
      setAdding(false);
    } finally {
      setBusy(false);
    }
  }

  async function handleTestPrimary(): Promise<void> {
    setTestingPrimary(true);
    try {
      const result = await onTest(service.activeKeyIndex);
      setTestNote(result.success ? `✓ ${result.message}` : `✗ ${result.message}`);
    } catch (err) {
      setTestNote(`✗ ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setTestingPrimary(false);
    }
  }

  async function handleTestAll(): Promise<void> {
    setTestingAll(true);
    try {
      let ok = 0;
      for (let index = 0; index < service.totalKeys; index += 1) {
        try {
          const result = await onTest(index);
          if (result.success) ok += 1;
        } catch {
          // Counted as a failure below; keep probing the rest.
        }
      }
      setTestNote(s.testAllSummary(ok, service.totalKeys));
    } finally {
      setTestingAll(false);
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
      onClose();
    } finally {
      setBusy(false);
      setConfirmingDelete(false);
    }
  }

  const metaEntries = Object.entries(service.meta ?? {});

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="max-h-[90vh] w-full max-w-xl overflow-y-auto rounded-2xl border border-white/[0.08] bg-[#0b0b0e]/95 p-5 shadow-[0_24px_80px_-12px_rgba(0,0,0,0.9)] backdrop-blur-xl"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-label={s.detailsFor(service.label)}
      >
        <div className="flex items-start gap-2.5">
          <span className="shrink-0 overflow-hidden rounded-lg border border-white/[0.07]">
            <BrandIcon presetId={presetId} size={40} />
          </span>
          <div className="min-w-0 flex-1">
            <h2 className="truncate text-base font-semibold text-zinc-100">{service.label}</h2>
            <p className="mt-0.5 truncate font-mono text-[11px] text-zinc-500">{service.id}</p>
            <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
              <span className="rounded bg-white/[0.06] px-1.5 py-0.5 text-[10.5px] font-medium text-zinc-300">
                {presetLabel}
              </span>
              <span className="text-[10.5px] text-zinc-500">
                {s.keysCount(service.totalKeys)} · {s.primaryShort(service.activeKeyIndex)}
              </span>
            </div>
          </div>
          <StatusDot
            health={service.health}
            label={s.healthName(service.health)}
            className="mt-1"
          />
          <button
            type="button"
            onClick={onClose}
            aria-label={s.closeDetails}
            className="shrink-0 rounded p-1 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200"
          >
            <X className="size-4" />
          </button>
        </div>

        {service.description !== "" && (
          <p className="mt-2.5 text-xs leading-relaxed text-zinc-400">{service.description}</p>
        )}

        {metaEntries.length > 0 && (
          <div className="mt-2.5 flex flex-wrap gap-1.5">
            {metaEntries.map(([name, value]) => (
              <span
                key={name}
                title={`${name}: ${value}`}
                className="max-w-full truncate rounded-md border border-white/[0.08] bg-white/[0.03] px-1.5 py-0.5 font-mono text-[10.5px] text-zinc-300"
              >
                {name}={value}
              </span>
            ))}
          </div>
        )}

        <p className="mt-4 text-[10px] font-semibold tracking-[0.14em] text-zinc-500 uppercase">
          {s.keysSection}
        </p>
        <div className="mt-2 space-y-1.5">
          {service.maskedKeys.map((masked, index) => (
            <ServiceKeyRow
              key={`${service.id}-${index}`}
              index={index}
              masked={masked}
              tag={service.keyTags?.[index] ?? null}
              keyHealth={service.keyHealth[index] ?? "healthy"}
              cooldownUntilMs={service.cooldownUntilMs[index] ?? 0}
              isPrimary={index === service.activeKeyIndex}
              canRemove={service.totalKeys > 1}
              onPromote={() => onPromote(index)}
              onRemove={() => onRemoveKey(index)}
              onTest={() => onTest(index)}
              onSaveTag={(tag) => onSaveTag(index, tag)}
              onReplaceKey={(secret) => onReplaceKey(index, secret)}
            />
          ))}
        </div>

        {adding ? (
          <div className="mt-2.5">
            <div className="flex gap-1.5">
              <input
                value={newKey}
                onChange={(event) => setNewKey(event.target.value)}
                placeholder={s.newKeyPlaceholder}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void handleAdd();
                }}
                spellCheck={false}
                className="min-w-0 flex-1 rounded-md border border-white/10 bg-zinc-950 px-2 py-1.5 font-mono text-xs text-zinc-100 placeholder:text-zinc-600"
              />
              <input
                value={newTag}
                onChange={(event) => setNewTag(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void handleAdd();
                }}
                placeholder={s.newTagPlaceholder}
                aria-label={s.newTagAria}
                spellCheck={false}
                className="w-24 shrink-0 rounded-md border border-white/10 bg-zinc-950 px-2 py-1.5 font-mono text-xs text-violet-200 placeholder:text-zinc-600"
              />
            </div>
            <div className="mt-1.5 flex gap-1.5">
              <button
                type="button"
                onClick={() => void handleAdd()}
                disabled={busy || newKey.trim().length === 0}
                className="rounded bg-emerald-600 px-2.5 py-1 text-xs font-medium text-white disabled:opacity-50"
              >
                {s.addToPool}
              </button>
              <button
                type="button"
                onClick={() => {
                  setAdding(false);
                  setNewKey("");
                  setNewTag("");
                }}
                className="rounded px-2.5 py-1 text-xs text-zinc-400 hover:bg-white/[0.06]"
              >
                {s.cancel}
              </button>
            </div>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setAdding(true)}
            className="mt-2 flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs text-zinc-400 transition hover:bg-white/[0.05] hover:text-zinc-100"
          >
            <Plus className="size-3.5" /> {s.addBackupKey}
          </button>
        )}

        {testNote !== null && (
          <p className="mt-2 truncate text-[11px] text-zinc-400" title={testNote}>
            {testNote}
          </p>
        )}

        {editing ? (
          <ServiceEditForm
            service={service}
            presets={presets}
            onSave={async (input) => {
              await onSaveService(input);
              setEditing(false);
            }}
            onCancel={() => setEditing(false)}
          />
        ) : null}

        <div className="mt-4 flex items-center gap-1.5 border-t border-white/[0.07] pt-3">
          {!editing && (
            <button
              type="button"
              onClick={() => setEditing(true)}
              title={s.editTitle}
              className={cn(
                "flex items-center gap-1 rounded px-1.5 py-1 text-[11.5px] transition",
                editing
                  ? "bg-white/[0.08] text-zinc-100"
                  : "text-zinc-400 hover:bg-white/[0.05] hover:text-zinc-100",
              )}
            >
              <Pencil className="size-3.5" />
              {s.editService}
            </button>
          )}
          <button
            type="button"
            onClick={() => void handleTestPrimary()}
            disabled={testingPrimary || busy}
            title={s.testPrimaryTitle}
            className="flex items-center gap-1 rounded px-1.5 py-1 text-[11.5px] text-zinc-400 transition hover:bg-white/[0.05] hover:text-zinc-100 disabled:opacity-50"
          >
            {testingPrimary ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <PlugZap className="size-3.5" />
            )}
            {s.testConnection}
          </button>
          <button
            type="button"
            onClick={() => void handleTestAll()}
            disabled={testingAll || testingPrimary || busy}
            title={s.testAllTitle}
            className="flex items-center gap-1 rounded px-1.5 py-1 text-[11.5px] text-zinc-400 transition hover:bg-white/[0.05] hover:text-zinc-100 disabled:opacity-50"
          >
            {testingAll ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <PlugZap className="size-3.5" />
            )}
            {s.testAll}
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
            {confirmingDelete ? s.confirmDelete : s.delete}
          </button>
        </div>
      </div>
    </div>
  );
}
