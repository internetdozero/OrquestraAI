import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Loader2, RotateCcw, Save, X } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import { ConfirmModal } from "./ConfirmModal";
import { PresetIconPicker } from "./PresetIconPicker";
import { PresetPickers } from "./PresetPickers";
import { presentPreset } from "../i18n/presetCopy";
import { ensureMcpCatalog } from "../lib/mcpCache";
import type { Preset } from "../lib/presets";
import { listSkills } from "../lib/tauri";

interface PresetEditModalProps {
  preset: Preset;
  saving?: boolean;
  onClose: () => void;
  onUpdate: (preset: Preset) => Promise<boolean>;
  onReset: (id: string) => Promise<boolean>;
}

export function PresetEditModal({
  preset,
  saving: externalSaving = false,
  onClose,
  onUpdate,
  onReset,
}: PresetEditModalProps) {
  const { t } = useI18n();
  const copy = presentPreset(t, preset);

  const [name, setName] = useState(preset.name);
  const [description, setDescription] = useState(preset.description);
  const [icon, setIcon] = useState(preset.icon);
  const [allSkills, setAllSkills] = useState(preset.allSkills);
  const [allMcps, setAllMcps] = useState(preset.allMcps);
  const [selectedSkills, setSelectedSkills] = useState<string[]>(preset.skills);
  const [selectedMcps, setSelectedMcps] = useState<string[]>(preset.mcps);

  const [availableSkills, setAvailableSkills] = useState<{ id: string; name: string }[]>([]);
  const [availableMcps, setAvailableMcps] = useState<{ id: string; name: string }[]>([]);
  const [search, setSearch] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isResetting, setIsResetting] = useState(false);
  const [confirmResetOpen, setConfirmResetOpen] = useState(false);

  useEffect(() => {
    let active = true;
    void listSkills("global")
      .then((items) => {
        if (active) setAvailableSkills(items.map((s) => ({ id: s.id, name: s.name })));
      })
      .catch(() => undefined);

    void ensureMcpCatalog()
      .then((cat) => {
        if (active) setAvailableMcps(cat.servers.map((s) => ({ id: s.id, name: s.name })));
      })
      .catch(() => undefined);

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !isSubmitting && !isResetting) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, isSubmitting, isResetting]);

  const toggleItem = (list: string[], setList: (v: string[]) => void, id: string) => {
    setList(list.includes(id) ? list.filter((i) => i !== id) : [...list, id]);
  };

  // Per-harness scopes ride along untouched: the editor owns the Global
  // baseline lists only, so saving never wipes harness specialization.
  const harnessScopeEntries = [
    ...Object.entries(preset.harnessSkills ?? {}).map(
      ([harness, ids]) => `${harness}: ${ids.length} skill${ids.length === 1 ? "" : "s"}`,
    ),
    ...Object.entries(preset.harnessMcps ?? {}).map(
      ([harness, ids]) => `${harness}: ${ids.length} MCP${ids.length === 1 ? "" : "s"}`,
    ),
  ];

  const handleSave = async () => {
    if (isSubmitting || name.trim().length === 0) return;
    setIsSubmitting(true);
    try {
      const updated: Preset = {
        ...preset,
        name: name.trim(),
        description: description.trim(),
        icon,
        allSkills,
        allMcps,
        skills: selectedSkills,
        mcps: selectedMcps,
      };
      if (await onUpdate(updated)) onClose();
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleReset = async () => {
    if (isResetting || !preset.builtin) return;
    setIsResetting(true);
    try {
      if (await onReset(preset.id)) onClose();
    } finally {
      setIsResetting(false);
    }
  };

  const busy = isSubmitting || isResetting || externalSaving;
  if (typeof document === "undefined") return null;

  return createPortal(
    <>
      <div
        role="dialog"
        aria-modal="true"
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 px-4 backdrop-blur-sm"
        onMouseDown={(e) => {
          if (e.target === e.currentTarget && !busy) onClose();
        }}
      >
        <div className="flex max-h-[90vh] w-full max-w-lg flex-col rounded-xl border border-white/10 bg-[#0d0d10] shadow-[0_20px_60px_-20px_rgba(0,0,0,0.9)]">
          <div className="flex items-start justify-between border-b border-white/[0.06] p-4">
            <div className="flex items-center gap-2">
              <h2 className="text-sm font-semibold text-zinc-100">{t.presets.editHeading}</h2>
              {preset.builtin && (
                <span className="rounded bg-emerald-500/15 px-1.5 py-0.5 text-[10px] font-medium text-emerald-400">
                  {t.presets.builtinBadge}
                </span>
              )}
            </div>
            <button
              type="button"
              onClick={onClose}
              aria-label={t.common.close}
              disabled={busy}
              className="grid size-6 shrink-0 place-items-center rounded text-zinc-500 hover:text-zinc-200"
            >
              <X className="size-3.5" />
            </button>
          </div>

          <form
            onSubmit={(e) => {
              e.preventDefault();
              void handleSave();
            }}
            className="flex min-h-0 flex-1 flex-col overflow-y-auto p-4 space-y-4"
          >
            <div>
              <label className="block text-[10px] font-medium tracking-[0.12em] text-zinc-400 uppercase">
                {t.presets.nameLabel}
              </label>
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t.presets.namePlaceholder}
                className="mt-1 h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-3 text-sm text-zinc-200 outline-none focus:border-emerald-500/40 focus:ring-2 focus:ring-emerald-500/10"
              />
            </div>

            <div>
              <label className="block text-[10px] font-medium tracking-[0.12em] text-zinc-400 uppercase">
                {t.presets.descriptionLabel}{" "}
                <span className="text-zinc-600">{t.presets.optional}</span>
              </label>
              <input
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={t.presets.descriptionPlaceholder}
                className="mt-1 h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-3 text-sm text-zinc-200 outline-none focus:border-emerald-500/40 focus:ring-2 focus:ring-emerald-500/10"
              />
            </div>

            <PresetIconPicker label={t.presets.iconLabel} value={icon} onChange={setIcon} />

            <PresetPickers
              t={t}
              search={search}
              onSearchChange={setSearch}
              availableSkills={availableSkills}
              selectedSkills={selectedSkills}
              onToggleSkill={(id) => toggleItem(selectedSkills, setSelectedSkills, id)}
              allSkills={allSkills}
              onAllSkillsChange={setAllSkills}
              availableMcps={availableMcps}
              selectedMcps={selectedMcps}
              onToggleMcp={(id) => toggleItem(selectedMcps, setSelectedMcps, id)}
              allMcps={allMcps}
              onAllMcpsChange={setAllMcps}
              harnessScopeEntries={harnessScopeEntries}
            />

            <div className="mt-4 flex items-center justify-between border-t border-white/[0.06] pt-4">
              {preset.builtin ? (
                <button
                  type="button"
                  onClick={() => setConfirmResetOpen(true)}
                  disabled={busy}
                  className="flex items-center gap-1.5 rounded-lg border border-white/10 px-2.5 py-1.5 text-[11.5px] font-medium text-zinc-400 hover:border-white/20 hover:text-zinc-200 disabled:opacity-50"
                >
                  {isResetting ? (
                    <Loader2 className="size-3.5 animate-spin" />
                  ) : (
                    <RotateCcw className="size-3.5" />
                  )}
                  {t.presets.resetToDefault}
                </button>
              ) : (
                <div />
              )}

              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={onClose}
                  disabled={busy}
                  className="rounded-lg border border-white/10 bg-white/[0.02] px-3 py-1.5 text-[11.5px] font-medium text-zinc-400 hover:text-zinc-200"
                >
                  {t.presets.cancel}
                </button>
                <button
                  type="submit"
                  disabled={busy || name.trim().length === 0}
                  className="flex items-center gap-1.5 rounded-lg border border-emerald-500/30 bg-emerald-500/15 px-3 py-1.5 text-[11.5px] font-medium text-emerald-200 hover:bg-emerald-500/25 disabled:opacity-50"
                >
                  {isSubmitting ? (
                    <Loader2 className="size-3.5 animate-spin" />
                  ) : (
                    <Save className="size-3.5" />
                  )}
                  {t.presets.editAction}
                </button>
              </div>
            </div>
          </form>
        </div>
      </div>
      {confirmResetOpen && (
        <ConfirmModal
          title={t.presets.resetToDefault}
          message={t.presets.resetConfirm(copy.name)}
          confirmLabel={t.presets.resetToDefault}
          cancelLabel={t.presets.cancel}
          tone="warning"
          busy={isResetting}
          onCancel={() => setConfirmResetOpen(false)}
          onConfirm={() => {
            setConfirmResetOpen(false);
            void handleReset();
          }}
        />
      )}
    </>,
    document.body,
  );
}
