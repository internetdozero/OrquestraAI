import { useState } from "react";
import { Check, ChevronDown, Plus, Trash2 } from "lucide-react";
import type { VaultPreset, VaultService } from "../../lib/vault.ts";
import { cn, confirmOnEnter } from "../../lib/utils.ts";
import { useI18n } from "../../hooks/useI18n.ts";
import { BrandIcon } from "../vault/BrandIcons.tsx";

export interface ServiceEditInput {
  label: string;
  preset: string;
  description: string;
  meta: Record<string, string>;
  customHeader?: string;
  customPrefix?: string;
  customEnv?: string;
}

interface ServiceEditFormProps {
  service: VaultService;
  presets: VaultPreset[];
  onSave: (input: ServiceEditInput) => Promise<void>;
  onCancel: () => void;
}

interface MetaRow {
  name: string;
  value: string;
}

/** Service-level editor: label, preset, description, meta rows, custom auth. */
export function ServiceEditForm({ service, presets, onSave, onCancel }: ServiceEditFormProps) {
  const [label, setLabel] = useState(service.label);
  const [presetId, setPresetId] = useState(service.preset);
  const [description, setDescription] = useState(service.description);
  const [metaRows, setMetaRows] = useState<MetaRow[]>(() =>
    Object.entries(service.meta ?? {}).map(([name, value]) => ({ name, value })),
  );
  const [customHeader, setCustomHeader] = useState(service.customHeader ?? "");
  const [customPrefix, setCustomPrefix] = useState(service.customPrefix ?? "");
  const [customEnv, setCustomEnv] = useState(service.customEnv ?? "");
  const [presetOpen, setPresetOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const { t } = useI18n();
  const s = t.credentials;
  const selectedPreset = presets.find((preset) => preset.id === presetId);

  async function handleSave(): Promise<void> {
    setError(null);
    if (label.trim() === "") {
      setError(s.labelRequired);
      return;
    }
    const meta: Record<string, string> = {};
    for (const row of metaRows) {
      if (row.name.trim() === "") continue;
      meta[row.name.trim()] = row.value;
    }
    setSaving(true);
    try {
      await onSave({
        label: label.trim(),
        preset: presetId,
        description: description.trim(),
        meta,
        customHeader: customHeader.trim() || undefined,
        customPrefix: customPrefix.trim() || undefined,
        customEnv: customEnv.trim() || undefined,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div
      className="mt-4 rounded-xl border border-white/[0.08] bg-white/[0.02] p-3"
      onKeyDown={(event) => confirmOnEnter(event, () => void handleSave())}
    >
      <p className="text-[10px] font-semibold tracking-[0.14em] text-zinc-500 uppercase">
        {s.editSection}
      </p>
      <div className="mt-2 grid grid-cols-2 gap-2">
        <input
          value={label}
          onChange={(event) => setLabel(event.target.value)}
          placeholder={s.editLabelPlaceholder}
          aria-label={s.editLabelAria}
          className="rounded-lg border border-white/10 bg-zinc-900 px-2.5 py-2 text-xs text-zinc-100 placeholder:text-zinc-600"
        />
        <div className="relative">
          <button
            type="button"
            onClick={() => setPresetOpen((current) => !current)}
            aria-label={s.editPresetAria}
            aria-expanded={presetOpen}
            className={cn(
              "flex w-full items-center gap-2 rounded-lg border bg-zinc-900 px-2.5 py-2 text-xs text-zinc-100 transition",
              presetOpen ? "border-white/25" : "border-white/10 hover:border-white/20",
            )}
          >
            {selectedPreset !== undefined && (
              <span className="shrink-0 overflow-hidden rounded-md">
                <BrandIcon presetId={selectedPreset.id} size={20} />
              </span>
            )}
            <span className="min-w-0 flex-1 truncate text-left">
              {selectedPreset?.label ?? presetId}
            </span>
            <ChevronDown className={cn("size-3.5 shrink-0 text-zinc-500 transition-transform", presetOpen && "rotate-180")} />
          </button>
          {presetOpen && (
            <>
              <div
                className="fixed inset-0 z-10"
                onClick={() => setPresetOpen(false)}
                role="presentation"
              />
              <ul
                role="listbox"
                aria-label={s.editPresetAria}
                className="absolute inset-x-0 top-full z-20 mt-1 max-h-56 overflow-y-auto rounded-lg border border-white/10 bg-zinc-900 p-1 shadow-[0_16px_48px_-8px_rgba(0,0,0,0.9)]"
              >
                {presets.map((preset) => (
                  <li key={preset.id}>
                    <button
                      type="button"
                      role="option"
                      aria-selected={preset.id === presetId}
                      onClick={() => {
                        setPresetId(preset.id);
                        // Custom auth shaping belongs to the previous preset.
                        setCustomHeader("");
                        setCustomPrefix("");
                        setCustomEnv("");
                        setPresetOpen(false);
                      }}
                      className={cn(
                        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition",
                        preset.id === presetId
                          ? "bg-emerald-500/15 text-emerald-200"
                          : "text-zinc-300 hover:bg-white/[0.06] hover:text-zinc-100",
                      )}
                    >
                      <span className="shrink-0 overflow-hidden rounded-md">
                        <BrandIcon presetId={preset.id} size={20} />
                      </span>
                      <span className="min-w-0 flex-1 truncate">{preset.label}</span>
                      {preset.id === presetId && <Check className="size-3.5 shrink-0" />}
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </div>
      </div>
      <input
        value={description}
        onChange={(event) => setDescription(event.target.value)}
        placeholder={s.editDescription}
        aria-label={s.editDescriptionAria}
        className="mt-2 w-full rounded-lg border border-white/10 bg-zinc-900 px-2.5 py-2 text-xs text-zinc-100 placeholder:text-zinc-600"
      />

      <p className="mt-3 text-[10px] font-semibold tracking-[0.14em] text-zinc-500 uppercase">
        {s.metaSection}
      </p>
      <div className="mt-1.5 space-y-1.5">
        {metaRows.map((row, index) => (
          <div key={index} className="flex items-center gap-1.5">
            <input
              value={row.name}
              onChange={(event) =>
                setMetaRows((current) =>
                  current.map((entry, position) =>
                    position === index ? { ...entry, name: event.target.value } : entry,
                  ),
                )
              }
              placeholder={s.metaNamePlaceholder}
              aria-label={s.metaNameAria(index)}
              autoComplete="off"
              spellCheck={false}
              className="w-32 shrink-0 rounded-md border border-white/10 bg-zinc-950 px-2 py-1.5 font-mono text-[11px] text-zinc-100 placeholder:text-zinc-600"
            />
            <input
              value={row.value}
              onChange={(event) =>
                setMetaRows((current) =>
                  current.map((entry, position) =>
                    position === index ? { ...entry, value: event.target.value } : entry,
                  ),
                )
              }
              placeholder={s.metaValuePlaceholder}
              aria-label={s.metaValueAria(index)}
              autoComplete="off"
              spellCheck={false}
              className="min-w-0 flex-1 rounded-md border border-white/10 bg-zinc-950 px-2 py-1.5 font-mono text-[11px] text-zinc-100 placeholder:text-zinc-600"
            />
            <button
              type="button"
              onClick={() => setMetaRows((current) => current.filter((_, position) => position !== index))}
              title={s.removeMetaAria}
              aria-label={s.removeMetaAria}
              className="shrink-0 rounded p-1 text-zinc-500 transition hover:bg-red-500/10 hover:text-red-300"
            >
              <Trash2 className="size-3.5" />
            </button>
          </div>
        ))}
      </div>
      <button
        type="button"
        onClick={() => setMetaRows((current) => [...current, { name: "", value: "" }])}
        className="mt-1.5 flex items-center gap-1 rounded px-1.5 py-1 text-[11.5px] text-zinc-400 transition hover:bg-white/[0.05] hover:text-zinc-100"
      >
        <Plus className="size-3.5" /> {s.addMeta}
      </button>

      <p className="mt-3 text-[10px] font-semibold tracking-[0.14em] text-zinc-500 uppercase">
        {s.customSection}
      </p>
      <div className="mt-1.5 grid grid-cols-3 gap-2">
        <input
          value={customHeader}
          onChange={(event) => setCustomHeader(event.target.value)}
          placeholder={s.customHeader}
          aria-label={s.customHeaderAria}
          autoComplete="off"
          spellCheck={false}
          className="rounded-md border border-white/10 bg-zinc-950 px-2 py-1.5 font-mono text-[11px] text-zinc-100 placeholder:text-zinc-600"
        />
        <input
          value={customPrefix}
          onChange={(event) => setCustomPrefix(event.target.value)}
          placeholder={s.customPrefix}
          aria-label={s.customPrefixAria}
          autoComplete="off"
          spellCheck={false}
          className="rounded-md border border-white/10 bg-zinc-950 px-2 py-1.5 font-mono text-[11px] text-zinc-100 placeholder:text-zinc-600"
        />
        <input
          value={customEnv}
          onChange={(event) => setCustomEnv(event.target.value)}
          placeholder={s.customEnv}
          aria-label={s.customEnvAria}
          autoComplete="off"
          spellCheck={false}
          className="rounded-md border border-white/10 bg-zinc-950 px-2 py-1.5 font-mono text-[11px] text-zinc-100 placeholder:text-zinc-600"
        />
      </div>

      {error !== null && <p className="mt-2 text-xs text-red-300">{error}</p>}
      <div className="mt-3 flex justify-end gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="rounded-lg px-3 py-1.5 text-sm text-zinc-400 hover:bg-white/[0.06]"
        >
          {s.cancel}
        </button>
        <button
          type="button"
          onClick={() => void handleSave()}
          disabled={saving}
          className="rounded-lg bg-emerald-600 px-4 py-1.5 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:opacity-50"
        >
          {saving ? s.saving : s.saveChanges}
        </button>
      </div>
    </div>
  );
}
