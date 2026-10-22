import { useMemo, useState } from "react";
import { Plus, Search } from "lucide-react";
import type { VaultPreset } from "../../lib/vault.ts";
import { confirmOnEnter } from "../../lib/utils.ts";
import { useI18n } from "../../hooks/useI18n.ts";
import { BrandIcon } from "./BrandIcons.tsx";
import { KeyInputRow, useKeyRows } from "./KeyInputRow.tsx";
import { PresetCard } from "./PresetCard.tsx";

type PresetCategory = "All" | "LLMs & AI" | "Cloud & Infra" | "Dev & Tools" | "Media & Images";

const CATEGORY_CHIPS: PresetCategory[] = [
  "All",
  "LLMs & AI",
  "Cloud & Infra",
  "Dev & Tools",
  "Media & Images",
];

/** Frontend-only grouping (the Rust catalog stays untouched). */
const PRESET_CATEGORY: Record<string, Exclude<PresetCategory, "All">> = {
  openai: "LLMs & AI",
  anthropic: "LLMs & AI",
  gemini: "LLMs & AI",
  openrouter: "LLMs & AI",
  groq: "LLMs & AI",
  mistral: "LLMs & AI",
  deepseek: "LLMs & AI",
  perplexity: "LLMs & AI",
  together: "LLMs & AI",
  cohere: "LLMs & AI",
  elevenlabs: "LLMs & AI",
  deepgram: "LLMs & AI",
  cloudflare: "Cloud & Infra",
  supabase: "Cloud & Infra",
  aws: "Cloud & Infra",
  vercel: "Cloud & Infra",
  neon: "Cloud & Infra",
  github: "Dev & Tools",
  gitlab: "Dev & Tools",
  stripe: "Dev & Tools",
  resend: "Dev & Tools",
  tavily: "Dev & Tools",
  exa: "Dev & Tools",
  jules: "Dev & Tools",
  bluesky: "Dev & Tools",
  pix: "Dev & Tools",
  unsplash: "Media & Images",
  pixabay: "Media & Images",
  pexels: "Media & Images",
  custom: "Dev & Tools",
};

interface SaveInput {
  id: string;
  label: string;
  preset: string;
  keys: string[];
  keyTags: (string | null)[];
  meta: Record<string, string>;
  customHeader?: string;
  customPrefix?: string;
  customEnv?: string;
}

interface AddCredentialModalProps {
  open: boolean;
  presets: VaultPreset[];
  onClose: () => void;
  onSave: (input: SaveInput) => Promise<void>;
}

/** New-service credential modal: brand grid, companion fields, key rows. Backup
 * keys join an existing pool from the pool's own card, not from here. */
export function AddCredentialModal({ open, presets, onClose, onSave }: AddCredentialModalProps) {
  const [presetSearch, setPresetSearch] = useState("");
  const [category, setCategory] = useState<PresetCategory>("All");
  const [presetId, setPresetId] = useState("github");
  const [serviceId, setServiceId] = useState("");
  const [label, setLabel] = useState("");
  const { rows, primaryIdx, setPrimaryIdx, patchRow, removeRow, resetRows, addRow } = useKeyRows();
  const [meta, setMeta] = useState<Record<string, string>>({});
  const [customHeader, setCustomHeader] = useState("");
  const [customPrefix, setCustomPrefix] = useState("");
  const [customEnv, setCustomEnv] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const { t } = useI18n();
  const s = t.credentials;

  const preset = useMemo(
    () => presets.find((entry) => entry.id === presetId) ?? presets[0],
    [presets, presetId],
  );
  const filtered = useMemo(() => {
    const needle = presetSearch.trim().toLowerCase();
    return presets.filter((entry) => {
      if (category !== "All" && (PRESET_CATEGORY[entry.id] ?? "Dev & Tools") !== category) {
        return false;
      }
      if (needle === "") return true;
      return (
        entry.label.toLowerCase().includes(needle) ||
        entry.description.toLowerCase().includes(needle)
      );
    });
  }, [presets, presetSearch, category]);

  if (!open) return null;

  async function handleSave(): Promise<void> {
    setError(null);
    const typed = rows
      .map((row) => ({ value: row.value.trim(), tag: row.tag.trim() || null }))
      .filter((row) => row.value !== "");
    if (typed.length === 0) {
      setError(s.needKey);
      return;
    }
    setSaving(true);
    try {
      if (serviceId.trim() === "") {
        setError(s.needServiceId);
        return;
      }
      const ordered = rows
        .map((row, position) => ({
          value: row.value.trim(),
          tag: row.tag.trim() || null,
          position,
        }))
        .filter((row) => row.value !== "")
        .sort((a, b) =>
          a.position === primaryIdx ? -1 : b.position === primaryIdx ? 1 : a.position - b.position,
        );
      await onSave({
        id: serviceId.trim(),
        label: label.trim() || serviceId.trim(),
        preset: preset?.id ?? "custom",
        keys: ordered.map((row) => row.value),
        keyTags: ordered.map((row) => row.tag),
        meta,
        customHeader: customHeader.trim() || undefined,
        customPrefix:
          customPrefix.trim().toLowerCase() === "none" || customPrefix.trim() === "-"
            ? ""
            : customPrefix.trim() || undefined,
        customEnv: customEnv.trim() || undefined,
      });
      setServiceId("");
      setLabel("");
      resetRows();
      setMeta({});
      setCustomHeader("");
      setCustomPrefix("");
      setCustomEnv("");
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="max-h-[90vh] w-full max-w-2xl overflow-y-auto rounded-2xl border border-white/[0.08] bg-[#0b0b0e]/95 p-5 shadow-[0_24px_80px_-12px_rgba(0,0,0,0.9)] backdrop-blur-xl"
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => confirmOnEnter(event, () => void handleSave())}
        role="dialog"
        aria-label={s.modalTitle}
      >
        <div className="flex items-center gap-2.5">
          {preset !== undefined && <BrandIcon presetId={preset.id} size={36} />}
          <div>
            <h2 className="text-base font-semibold text-zinc-100">{s.modalTitle}</h2>
            <p className="text-[11.5px] text-zinc-500">{s.modalSubtitle}</p>
          </div>
        </div>

        <p className="mt-4 text-[10px] font-semibold tracking-[0.14em] text-zinc-500 uppercase">
          {s.chooseProvider}
        </p>
        <div className="mt-2 flex flex-wrap gap-1.5" role="group" aria-label={s.filterCategoryAria}>
          {CATEGORY_CHIPS.map((chip) => (
            <button
              key={chip}
              type="button"
              onClick={() => setCategory(chip)}
              className={
                category === chip
                  ? "rounded-full border border-emerald-500/50 bg-emerald-500/15 px-3 py-1 text-[11.5px] font-medium text-emerald-200"
                  : "rounded-full border border-white/10 bg-white/[0.02] px-3 py-1 text-[11.5px] text-zinc-400 transition hover:border-white/20 hover:text-zinc-200"
              }
            >
              {chip === "All" ? s.allCategory : (s.categoryNames[chip] ?? chip)}
            </button>
          ))}
        </div>
        <div className="relative mt-2.5">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-zinc-500" />
          <input
            value={presetSearch}
            onChange={(event) => setPresetSearch(event.target.value)}
            placeholder={s.searchProviders}
            aria-label={s.searchProvidersAria}
            data-no-enter
            className="w-full rounded-lg border border-white/10 bg-zinc-900 py-2 pr-2 pl-8 text-[12.5px] text-zinc-100 placeholder:text-zinc-600"
          />
        </div>
        <div className="mt-2 grid max-h-72 grid-cols-1 gap-1.5 overflow-y-auto px-0.5 py-0.5 [-webkit-mask-image:linear-gradient(to_bottom,black_88%,transparent)] [mask-image:linear-gradient(to_bottom,black_88%,transparent)] sm:grid-cols-2">
          {filtered.map((entry) => (
            <PresetCard
              key={entry.id}
              preset={entry}
              selected={entry.id === preset?.id}
              onSelect={() => {
                setPresetId(entry.id);
                setMeta({});
              }}
            />
          ))}
          {filtered.length === 0 && (
            <p className="col-span-full py-3 text-center text-xs text-zinc-500">
              {s.noProviderMatch}
            </p>
          )}
        </div>
        <div className="mt-4 rounded-xl border border-emerald-500/20 bg-emerald-500/[0.03] p-3">
          <p className="text-[10px] font-semibold tracking-[0.14em] text-zinc-500 uppercase">
            {s.configureService(preset?.label ?? "service")}
          </p>
          <div className="mt-2.5 grid grid-cols-2 gap-2">
            <input
              value={serviceId}
              onChange={(event) => setServiceId(event.target.value)}
              placeholder={s.serviceIdPlaceholder}
              aria-label={s.serviceIdAria}
              className="rounded-lg border border-white/10 bg-zinc-900 px-2.5 py-2 font-mono text-xs text-zinc-100 placeholder:text-zinc-600"
            />
            <input
              value={label}
              onChange={(event) => setLabel(event.target.value)}
              placeholder={s.labelPlaceholder}
              aria-label={s.labelAria}
              className="rounded-lg border border-white/10 bg-zinc-900 px-2.5 py-2 text-xs text-zinc-100 placeholder:text-zinc-600"
            />
          </div>
          {preset !== undefined &&
            preset.metaFields.map((field) => (
              <input
                key={field.key}
                value={meta[field.key] ?? ""}
                onChange={(event) =>
                  setMeta((current) => ({
                    ...current,
                    [field.key]: event.target.value,
                  }))
                }
                placeholder={`${field.label} — ${field.placeholder}`}
                aria-label={field.label}
                className="mt-2 w-full rounded-lg border border-white/10 bg-zinc-900 px-2.5 py-2 font-mono text-xs text-zinc-100 placeholder:text-zinc-600"
              />
            ))}
          {preset?.id === "custom" && (
            <div className="mt-2 grid grid-cols-3 gap-2">
              <input
                value={customHeader}
                onChange={(event) => setCustomHeader(event.target.value)}
                placeholder={s.customHeader}
                aria-label={s.customHeaderAria}
                className="rounded-lg border border-white/10 bg-zinc-900 px-2.5 py-2 font-mono text-xs text-zinc-100 placeholder:text-zinc-600"
              />
              <input
                value={customPrefix}
                onChange={(event) => setCustomPrefix(event.target.value)}
                placeholder={s.customPrefix}
                aria-label={s.customPrefixAria}
                className="rounded-lg border border-white/10 bg-zinc-900 px-2.5 py-2 font-mono text-xs text-zinc-100 placeholder:text-zinc-600"
              />
              <input
                value={customEnv}
                onChange={(event) => setCustomEnv(event.target.value)}
                placeholder={s.customEnv}
                aria-label={s.customEnvAria}
                className="rounded-lg border border-white/10 bg-zinc-900 px-2.5 py-2 font-mono text-xs text-zinc-100 placeholder:text-zinc-600"
              />
            </div>
          )}
        </div>

        <p className="mt-4 text-[10px] font-semibold tracking-[0.14em] text-zinc-500 uppercase">
          {s.keysStep}
        </p>
        <div className="mt-2 space-y-1.5">
          {rows.map((row, index) => (
            <KeyInputRow
              key={index}
              value={row.value}
              tag={row.tag}
              index={index}
              isPrimary={index === primaryIdx}
              canRemove={rows.length > 1}
              onChange={(value) => patchRow(index, { value })}
              onTagChange={(tag) => patchRow(index, { tag })}
              onMakePrimary={() => setPrimaryIdx(index)}
              onRemove={() => removeRow(index)}
              revealed={row.revealed}
              onToggleReveal={() => patchRow(index, { revealed: !row.revealed })}
            />
          ))}
        </div>
        <button
          type="button"
          onClick={addRow}
          className="mt-2 flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs text-zinc-400 transition hover:bg-white/[0.05] hover:text-zinc-100"
        >
          <Plus className="size-3.5" /> {s.addBackupKey}
        </button>
        {preset !== undefined && preset.keyUrl !== "" && (
          <a
            href={preset.keyUrl}
            target="_blank"
            rel="noreferrer"
            className="mt-1 inline-block text-[11.5px] text-emerald-300/90 hover:text-emerald-200"
          >
            {s.getKeyLink(preset.label)}
          </a>
        )}
        {error !== null && <p className="mt-2 text-xs text-red-300">{error}</p>}

        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
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
            {saving ? s.saving : s.saveService}
          </button>
        </div>
      </div>
    </div>
  );
}
