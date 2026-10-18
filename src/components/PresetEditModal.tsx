import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { Cpu, Globe, Loader2, RotateCcw, Save, Search, ShieldAlert, Sparkles, X, Zap } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import { presentPreset } from "../i18n/presetCopy";
import { ensureMcpCatalog } from "../lib/mcpCache";
import type { Preset } from "../lib/presets";
import { listSkills } from "../lib/tauri";
import { cn } from "../lib/utils";

interface PresetEditModalProps {
  preset: Preset;
  saving?: boolean;
  onClose: () => void;
  onUpdate: (preset: Preset) => Promise<boolean>;
  onReset: (id: string) => Promise<boolean>;
}

const AVAILABLE_ICONS = [
  { key: "shield", icon: ShieldAlert, label: "Shield" },
  { key: "cpu", icon: Cpu, label: "CPU" },
  { key: "globe", icon: Globe, label: "Globe" },
  { key: "zap", icon: Zap, label: "Zap" },
  { key: "custom", icon: Sparkles, label: "Sparkles" },
];

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

  useEffect(() => {
    let active = true;
    void listSkills("global").then((items) => {
      if (active) setAvailableSkills(items.map((s) => ({ id: s.id, name: s.name })));
    }).catch(() => undefined);

    void ensureMcpCatalog().then((cat) => {
      if (active) setAvailableMcps(cat.servers.map((s) => ({ id: s.id, name: s.name })));
    }).catch(() => undefined);

    return () => { active = false; };
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
    if (!window.confirm(t.presets.resetConfirm(copy.name))) return;
    setIsResetting(true);
    try {
      if (await onReset(preset.id)) onClose();
    } finally {
      setIsResetting(false);
    }
  };

  // Live filter over both pickers. Checked items always stay visible so a
  // selection made before typing can never be lost behind the query.
  const needle = search.trim().toLowerCase();
  const matches = (id: string, name: string, selected: boolean) =>
    selected ||
    needle === "" ||
    id.toLowerCase().includes(needle) ||
    name.toLowerCase().includes(needle);
  const shownSkills = useMemo(
    () => availableSkills.filter((s) => matches(s.id, s.name, selectedSkills.includes(s.id))),
    [availableSkills, needle, selectedSkills],
  );
  const shownMcps = useMemo(
    () => availableMcps.filter((m) => matches(m.id, m.name, selectedMcps.includes(m.id))),
    [availableMcps, needle, selectedMcps],
  );

  const busy = isSubmitting || isResetting || externalSaving;
  if (typeof document === "undefined") return null;

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 px-4 backdrop-blur-sm"
      onMouseDown={(e) => { if (e.target === e.currentTarget && !busy) onClose(); }}
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
          onSubmit={(e) => { e.preventDefault(); void handleSave(); }}
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
              {t.presets.descriptionLabel} <span className="text-zinc-600">{t.presets.optional}</span>
            </label>
            <input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t.presets.descriptionPlaceholder}
              className="mt-1 h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-3 text-sm text-zinc-200 outline-none focus:border-emerald-500/40 focus:ring-2 focus:ring-emerald-500/10"
            />
          </div>

          <div>
            <label className="block text-[10px] font-medium tracking-[0.12em] text-zinc-400 uppercase">
              {t.presets.iconLabel}
            </label>
            <div className="mt-1.5 flex gap-2">
              {AVAILABLE_ICONS.map((item) => {
                const IconComp = item.icon;
                const isSelected = icon === item.key;
                return (
                  <button
                    key={item.key}
                    type="button"
                    onClick={() => setIcon(item.key)}
                    className={cn(
                      "flex size-9 items-center justify-center rounded-lg border transition-colors",
                      isSelected
                        ? "border-emerald-500/50 bg-emerald-500/20 text-emerald-300"
                        : "border-white/10 bg-white/[0.02] text-zinc-400 hover:border-white/20 hover:text-zinc-200",
                    )}
                    title={item.label}
                  >
                    <IconComp className="size-4" />
                  </button>
                );
              })}
            </div>
          </div>

          <div className="relative">
            <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-zinc-500" />
            <input
              type="search"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={t.presets.filterItems}
              aria-label={t.presets.filterItems}
              spellCheck={false}
              className="h-8 w-full rounded-lg border border-white/10 bg-zinc-900/60 pr-3 pl-8 text-xs text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-emerald-500/40 [&::-webkit-search-cancel-button]:appearance-none"
            />
          </div>

          <div className="space-y-2 border-t border-white/[0.06] pt-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-medium text-zinc-200">
                Skills ({allSkills ? t.presets.everySkill : selectedSkills.length})
              </span>
              <label className="flex cursor-pointer items-center gap-1.5 text-xs text-zinc-400">
                <input
                  type="checkbox"
                  checked={allSkills}
                  onChange={(e) => setAllSkills(e.target.checked)}
                  className="rounded border-zinc-700 bg-zinc-900 text-emerald-500 focus:ring-0"
                />
                {t.presets.allSkillsSwitch}
              </label>
            </div>
            {!allSkills && (
              <div className="max-h-28 overflow-y-auto rounded-lg border border-white/10 bg-zinc-900/30 p-2 space-y-1">
                {shownSkills.map((s) => (
                  <label key={s.id} className="flex cursor-pointer items-center gap-2 rounded px-1.5 py-1 text-xs text-zinc-300 hover:bg-white/[0.03]">
                    <input
                      type="checkbox"
                      checked={selectedSkills.includes(s.id)}
                      onChange={() => toggleItem(selectedSkills, setSelectedSkills, s.id)}
                      className="rounded border-zinc-700 bg-zinc-900 text-emerald-500 focus:ring-0"
                    />
                    <span className="truncate">{s.name}</span>
                    <span className="text-[10px] text-zinc-600">({s.id})</span>
                  </label>
                ))}
              </div>
            )}
          </div>

          {harnessScopeEntries.length > 0 && (
            <div className="rounded-lg border border-white/10 bg-zinc-900/30 px-2.5 py-2 text-[11px] text-zinc-400">
              {t.presets.harnessScopesKept}: {harnessScopeEntries.join(" · ")}
            </div>
          )}

          <div className="space-y-2 border-t border-white/[0.06] pt-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-medium text-zinc-200">
                MCP Servers ({allMcps ? t.presets.everyMcp : selectedMcps.length})
              </span>
              <label className="flex cursor-pointer items-center gap-1.5 text-xs text-zinc-400">
                <input
                  type="checkbox"
                  checked={allMcps}
                  onChange={(e) => setAllMcps(e.target.checked)}
                  className="rounded border-zinc-700 bg-zinc-900 text-emerald-500 focus:ring-0"
                />
                {t.presets.allMcpsSwitch}
              </label>
            </div>
            {!allMcps && (
              <div className="max-h-28 overflow-y-auto rounded-lg border border-white/10 bg-zinc-900/30 p-2 space-y-1">
                {shownMcps.map((m) => (
                  <label key={m.id} className="flex cursor-pointer items-center gap-2 rounded px-1.5 py-1 text-xs text-zinc-300 hover:bg-white/[0.03]">
                    <input
                      type="checkbox"
                      checked={selectedMcps.includes(m.id)}
                      onChange={() => toggleItem(selectedMcps, setSelectedMcps, m.id)}
                      className="rounded border-zinc-700 bg-zinc-900 text-emerald-500 focus:ring-0"
                    />
                    <span className="truncate">{m.name}</span>
                    <span className="text-[10px] text-zinc-600">({m.id})</span>
                  </label>
                ))}
              </div>
            )}
          </div>

          <div className="mt-4 flex items-center justify-between border-t border-white/[0.06] pt-4">
            {preset.builtin ? (
              <button
                type="button"
                onClick={() => void handleReset()}
                disabled={busy}
                className="flex items-center gap-1.5 rounded-lg border border-white/10 px-2.5 py-1.5 text-[11.5px] font-medium text-zinc-400 hover:border-white/20 hover:text-zinc-200 disabled:opacity-50"
              >
                {isResetting ? <Loader2 className="size-3.5 animate-spin" /> : <RotateCcw className="size-3.5" />}
                {t.presets.resetToDefault}
              </button>
            ) : <div />}

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
                {isSubmitting ? <Loader2 className="size-3.5 animate-spin" /> : <Save className="size-3.5" />}
                {t.presets.editAction}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>,
    document.body,
  );
}
