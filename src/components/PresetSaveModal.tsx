import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Loader2, Save, X } from "lucide-react";
import { useI18n } from "../hooks/useI18n";

interface PresetSaveModalProps {
  saving: boolean;
  onClose: () => void;
  /** Resolves `true` when the preset was saved, so the modal can close. */
  onSave: (name: string, description: string) => Promise<boolean>;
}

/**
 * Captures the live skill and MCP combination under a name. The shell shows
 * this only from the preset bar's "Save" button; the Rust command reads the
 * current state itself, so the form only has to name it.
 */
export function PresetSaveModal({ saving, onClose, onSave }: PresetSaveModalProps) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const submit = async () => {
    if (saving || name.trim().length === 0) return;
    if (await onSave(name.trim(), description)) onClose();
  };

  if (typeof document === "undefined") return null;

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 px-4 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !saving) onClose();
      }}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
        className="w-full max-w-sm rounded-xl border border-white/10 bg-[#0d0d10] p-4 shadow-[0_20px_60px_-20px_rgba(0,0,0,0.9)]"
      >
        <div className="flex items-start justify-between gap-3">
          <div>
            <h2 className="text-sm font-semibold text-zinc-100">{t.presets.saveHeading}</h2>
            <p className="mt-0.5 text-[11px] leading-relaxed text-zinc-500">{t.presets.saveBody}</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t.common.close}
            className="grid size-6 shrink-0 place-items-center rounded text-zinc-500 transition-colors hover:text-zinc-200"
          >
            <X className="size-3.5" />
          </button>
        </div>

        <label className="mt-4 block">
          <span className="text-[10px] font-medium tracking-[0.12em] text-zinc-500 uppercase">
            {t.presets.nameLabel}
          </span>
          <input
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={t.presets.namePlaceholder}
            spellCheck={false}
            className="mt-1 h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-3 text-sm text-zinc-200 placeholder:text-zinc-600 outline-none transition-colors focus:border-emerald-500/40 focus:ring-2 focus:ring-emerald-500/10"
          />
        </label>

        <label className="mt-3 block">
          <span className="text-[10px] font-medium tracking-[0.12em] text-zinc-500 uppercase">
            {t.presets.descriptionLabel} <span className="text-zinc-700">{t.presets.optional}</span>
          </span>
          <input
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder={t.presets.descriptionPlaceholder}
            spellCheck={false}
            className="mt-1 h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-3 text-sm text-zinc-200 placeholder:text-zinc-600 outline-none transition-colors focus:border-emerald-500/40 focus:ring-2 focus:ring-emerald-500/10"
          />
        </label>

        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg border border-white/10 bg-white/[0.02] px-3 py-1.5 text-[11.5px] font-medium text-zinc-400 transition-colors hover:text-zinc-200"
          >
            {t.presets.cancel}
          </button>
          <button
            type="submit"
            disabled={saving || name.trim().length === 0}
            className="flex items-center gap-1.5 rounded-lg border border-emerald-500/30 bg-emerald-500/15 px-3 py-1.5 text-[11.5px] font-medium text-emerald-200 transition-colors hover:bg-emerald-500/25 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {saving ? <Loader2 className="size-3.5 animate-spin" /> : <Save className="size-3.5" />}
            {t.presets.saveAction}
          </button>
        </div>
      </form>
    </div>,
    document.body,
  );
}
