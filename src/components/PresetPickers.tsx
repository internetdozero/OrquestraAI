import { useCallback, useMemo } from "react";
import { Search } from "lucide-react";
import type { useI18n } from "../hooks/useI18n";

/** One row in a picker list. */
export interface PickerItem {
  id: string;
  name: string;
}

type Copy = ReturnType<typeof useI18n>["t"];

interface PickerSectionProps {
  title: string;
  allChecked: boolean;
  onAllChange: (value: boolean) => void;
  allLabel: string;
  items: PickerItem[];
  selected: string[];
  onToggle: (id: string) => void;
}

/** One "everything or pick some" checkbox list: header switch plus rows. */
function PickerSection({
  title,
  allChecked,
  onAllChange,
  allLabel,
  items,
  selected,
  onToggle,
}: PickerSectionProps) {
  return (
    <div className="space-y-2 border-t border-white/[0.06] pt-3">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-zinc-200">{title}</span>
        <label className="flex cursor-pointer items-center gap-1.5 text-xs text-zinc-400">
          <input
            type="checkbox"
            checked={allChecked}
            onChange={(e) => onAllChange(e.target.checked)}
            className="rounded border-zinc-700 bg-zinc-900 text-emerald-500 focus:ring-0"
          />
          {allLabel}
        </label>
      </div>
      {!allChecked && (
        <div className="max-h-28 overflow-y-auto rounded-lg border border-white/10 bg-zinc-900/30 p-2 space-y-1">
          {items.map((item) => (
            <label
              key={item.id}
              className="flex cursor-pointer items-center gap-2 rounded px-1.5 py-1 text-xs text-zinc-300 hover:bg-white/[0.03]"
            >
              <input
                type="checkbox"
                checked={selected.includes(item.id)}
                onChange={() => onToggle(item.id)}
                className="rounded border-zinc-700 bg-zinc-900 text-emerald-500 focus:ring-0"
              />
              <span className="truncate">{item.name}</span>
              <span className="text-[10px] text-zinc-600">({item.id})</span>
            </label>
          ))}
        </div>
      )}
    </div>
  );
}

export interface PresetPickersProps {
  t: Copy;
  search: string;
  onSearchChange: (value: string) => void;
  availableSkills: PickerItem[];
  selectedSkills: string[];
  onToggleSkill: (id: string) => void;
  allSkills: boolean;
  onAllSkillsChange: (value: boolean) => void;
  availableMcps: PickerItem[];
  selectedMcps: string[];
  onToggleMcp: (id: string) => void;
  allMcps: boolean;
  onAllMcpsChange: (value: boolean) => void;
  harnessScopeEntries: string[];
}

/**
 * Search filter plus the skills and MCP picker sections. Owns the
 * live-filter matching so the modal stays a state container; checked
 * items always stay visible so a selection made before typing can
 * never be lost behind the query.
 */
export function PresetPickers({
  t,
  search,
  onSearchChange,
  availableSkills,
  selectedSkills,
  onToggleSkill,
  allSkills,
  onAllSkillsChange,
  availableMcps,
  selectedMcps,
  onToggleMcp,
  allMcps,
  onAllMcpsChange,
  harnessScopeEntries,
}: PresetPickersProps) {
  const needle = search.trim().toLowerCase();
  const matches = useCallback(
    (id: string, name: string, selected: boolean) =>
      selected ||
      needle === "" ||
      id.toLowerCase().includes(needle) ||
      name.toLowerCase().includes(needle),
    [needle],
  );
  const shownSkills = useMemo(
    () => availableSkills.filter((s) => matches(s.id, s.name, selectedSkills.includes(s.id))),
    [availableSkills, matches, selectedSkills],
  );
  const shownMcps = useMemo(
    () => availableMcps.filter((m) => matches(m.id, m.name, selectedMcps.includes(m.id))),
    [availableMcps, matches, selectedMcps],
  );

  return (
    <>
      <div className="relative">
        <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-zinc-500" />
        <input
          type="search"
          value={search}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder={t.presets.filterItems}
          aria-label={t.presets.filterItems}
          spellCheck={false}
          className="h-8 w-full rounded-lg border border-white/10 bg-zinc-900/60 pr-3 pl-8 text-xs text-zinc-200 outline-none placeholder:text-zinc-600 focus:border-emerald-500/40 [&::-webkit-search-cancel-button]:appearance-none"
        />
      </div>

      <PickerSection
        title={`Skills (${allSkills ? t.presets.everySkill : selectedSkills.length})`}
        allChecked={allSkills}
        onAllChange={onAllSkillsChange}
        allLabel={t.presets.allSkillsSwitch}
        items={shownSkills}
        selected={selectedSkills}
        onToggle={onToggleSkill}
      />

      {harnessScopeEntries.length > 0 && (
        <div className="rounded-lg border border-white/10 bg-zinc-900/30 px-2.5 py-2 text-[11px] text-zinc-400">
          {t.presets.harnessScopesKept}: {harnessScopeEntries.join(" · ")}
        </div>
      )}

      <PickerSection
        title={`MCP Servers (${allMcps ? t.presets.everyMcp : selectedMcps.length})`}
        allChecked={allMcps}
        onAllChange={onAllMcpsChange}
        allLabel={t.presets.allMcpsSwitch}
        items={shownMcps}
        selected={selectedMcps}
        onToggle={onToggleMcp}
      />
    </>
  );
}
