import { useState } from "react";
import { Compass, Loader2, Pencil, Plus, X } from "lucide-react";
import { presentPreset, summarizePreset } from "../i18n/presetCopy";
import { presetIcon, type Preset } from "../lib/presets";
import { cn } from "../lib/utils";
import { useI18n } from "../hooks/useI18n";
import { PresetSaveModal } from "./PresetSaveModal";
import { PresetEditModal } from "./PresetEditModal";

interface PresetBarProps {
  presets: Preset[];
  loading: boolean;
  /** Preset currently being applied, if any. */
  busyId: string | null;
  saving: boolean;
  onApply: (preset: Preset) => void;
  onSave: (name: string, description: string) => Promise<boolean>;
  onUpdate: (preset: Preset) => Promise<boolean>;
  onReset: (id: string) => Promise<boolean>;
  onDelete: (preset: Preset) => void;
}

function PresetPill({
  preset,
  busy,
  onApply,
  onEdit,
  onDelete,
}: {
  preset: Preset;
  busy: boolean;
  onApply: (preset: Preset) => void;
  onEdit: (preset: Preset) => void;
  onDelete: (preset: Preset) => void;
}) {
  const { t } = useI18n();
  const copy = presentPreset(t, preset);
  const Icon = presetIcon(preset.icon);

  return (
    <div
      className={cn(
        "group flex shrink-0 items-center rounded-lg border transition-colors",
        preset.active
          ? "accent-border accent-bg-soft accent-glow border"
          : "border-white/10 bg-white/[0.02] hover:border-white/20",
      )}
    >
      <button
        type="button"
        onClick={() => onApply(preset)}
        disabled={busy}
        title={`${copy.name} — ${copy.description} (${summarizePreset(t, preset)})`}
        className={cn(
          "flex items-center gap-1.5 rounded-lg py-1.5 pr-2.5 pl-2.5 text-[11.5px] font-medium transition-colors disabled:cursor-wait",
          preset.active ? "accent-text" : "text-zinc-300 hover:text-zinc-100",
        )}
      >
        {busy ? (
          <Loader2 className="size-3.5 animate-spin" />
        ) : (
          <Icon
            className={cn(
              "size-3.5",
              preset.active ? "accent-text" : "text-zinc-400",
            )}
          />
        )}
        <span className="whitespace-nowrap">{copy.name}</span>
        {preset.active && !busy && (
          <span className="accent-dot size-1.5 rounded-full" />
        )}
      </button>

      <button
        type="button"
        onClick={() => onEdit(preset)}
        title={t.presets.editTitle(copy.name)}
        aria-label={t.presets.editTitle(copy.name)}
        className="grid h-5 w-0 place-items-center overflow-hidden rounded text-zinc-500 opacity-0 pointer-events-none transition-[width,opacity] duration-150 delay-0 group-hover:mr-0.5 group-hover:w-5 group-hover:opacity-100 group-hover:pointer-events-auto group-hover:delay-75 hover:text-emerald-300 focus-visible:mr-0.5 focus-visible:w-5 focus-visible:opacity-100 focus-visible:pointer-events-auto"
      >
        <Pencil className="size-3" />
      </button>

      {!preset.builtin && (
        <button
          type="button"
          onClick={() => onDelete(preset)}
          title={t.presets.deleteTitle(copy.name)}
          aria-label={t.presets.deleteTitle(copy.name)}
          className="grid h-5 w-0 place-items-center overflow-hidden rounded text-zinc-600 opacity-0 pointer-events-none transition-[width,opacity] duration-150 delay-0 group-hover:mr-1 group-hover:w-5 group-hover:opacity-100 group-hover:pointer-events-auto group-hover:delay-75 hover:text-rose-300 focus-visible:mr-1 focus-visible:w-5 focus-visible:opacity-100 focus-visible:pointer-events-auto"
        >
          <X className="size-3" />
        </button>
      )}
    </div>
  );
}

/**
 * One-click mission profiles, shown in every view because a preset rewrites
 * both the skill projections and the MCP catalog. The active profile carries an
 * emerald glow; custom profiles can be deleted from the pill itself.
 */
export function PresetBar({
  presets,
  loading,
  busyId,
  saving,
  onApply,
  onSave,
  onUpdate,
  onReset,
  onDelete,
}: PresetBarProps) {
  const { t } = useI18n();
  const [saveOpen, setSaveOpen] = useState(false);
  const [editingPreset, setEditingPreset] = useState<Preset | null>(null);
  const showSkeleton = loading && presets.length === 0;

  return (
    <div className="relative z-10 shrink-0 border-b border-white/[0.05] bg-[#0b0b0e]/60 backdrop-blur">
      <div className="flex items-center gap-2 px-4 py-2 sm:px-6">
        <span className="hidden shrink-0 items-center gap-1.5 text-[10px] font-medium tracking-[0.14em] text-zinc-500 uppercase sm:flex">
          <Compass className="size-3 text-emerald-400/80" />
          {t.presets.title}
        </span>

        <div className="flex min-w-0 flex-1 items-center gap-1.5 overflow-x-auto pb-0.5">
          {showSkeleton
            ? Array.from({ length: 4 }, (_, index) => (
                <div
                  key={index}
                  className="h-7 w-24 shrink-0 animate-pulse rounded-lg border border-white/[0.06] bg-white/[0.03]"
                />
              ))
            : presets.map((preset) => (
                <PresetPill
                  key={preset.id}
                  preset={preset}
                  busy={busyId === preset.id}
                  onApply={onApply}
                  onEdit={setEditingPreset}
                  onDelete={onDelete}
                />
              ))}
        </div>

        <button
          type="button"
          onClick={() => setSaveOpen(true)}
          title={t.presets.saveTitle}
          className="flex shrink-0 items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.02] px-2.5 py-1.5 text-[11.5px] font-medium text-zinc-400 transition-colors hover:border-white/20 hover:text-zinc-200"
        >
          <Plus className="size-3.5" />
          <span className="hidden sm:inline">{t.presets.save}</span>
        </button>
      </div>

      {saveOpen && (
        <PresetSaveModal
          saving={saving}
          onClose={() => setSaveOpen(false)}
          onSave={onSave}
        />
      )}

      {editingPreset && (
        <PresetEditModal
          preset={editingPreset}
          onClose={() => setEditingPreset(null)}
          onUpdate={onUpdate}
          onReset={onReset}
        />
      )}
    </div>
  );
}
