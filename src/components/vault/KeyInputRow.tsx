import { useState } from "react";
import { Crown, Eye, EyeOff, Trash2 } from "lucide-react";
import { cn } from "../../lib/utils.ts";
import { useI18n } from "../../hooks/useI18n.ts";

interface KeyInputRowProps {
  value: string;
  tag: string;
  index: number;
  isPrimary: boolean;
  canRemove: boolean;
  onChange: (value: string) => void;
  onTagChange: (tag: string) => void;
  onMakePrimary: () => void;
  onRemove: () => void;
  revealed: boolean;
  onToggleReveal: () => void;
}

/**
 * One editable key row for the credential modal: masked input with an
 * eye toggle, primary badge / promote action, and a delete button.
 * Stored pool keys stay masked by the blind backend; only freshly typed
 * values can be revealed, and only in this input.
 */
export function KeyInputRow({
  value,
  tag,
  index,
  isPrimary,
  canRemove,
  onChange,
  onTagChange,
  onMakePrimary,
  onRemove,
  revealed,
  onToggleReveal,
}: KeyInputRowProps) {
  const { t } = useI18n();
  const s = t.credentials;
  return (
    <div
      className={cn(
        "flex items-center gap-1.5 rounded-lg border px-2 py-1.5 transition-colors",
        isPrimary
          ? "border-emerald-500/50 bg-emerald-500/[0.07]"
          : "border-white/[0.08] bg-white/[0.02] focus-within:border-white/[0.2]",
      )}
    >
      {isPrimary ? (
        <span className="flex shrink-0 items-center gap-1 rounded bg-emerald-500/20 px-1.5 py-0.5 text-[10px] font-semibold text-emerald-300">
          <Crown className="size-3" /> {s.primaryAction}
        </span>
      ) : (
        <span className="shrink-0 font-mono text-[10px] text-zinc-500">{s.backupIndex(index)}</span>
      )}
      <input
        type={revealed ? "text" : "password"}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={isPrimary ? s.primaryKeyPlaceholder : s.backupKeyPlaceholder}
        autoComplete="off"
        spellCheck={false}
        className="min-w-0 flex-1 bg-transparent font-mono text-xs text-zinc-100 outline-none placeholder:text-zinc-600"
      />
      <input
        value={tag}
        onChange={(event) => onTagChange(event.target.value)}
        placeholder={s.tagPlaceholder}
        aria-label={isPrimary ? s.tagAriaPrimary : s.tagAriaBackup(index)}
        autoComplete="off"
        spellCheck={false}
        className="w-28 shrink-0 rounded-md border border-white/10 bg-zinc-950/70 px-1.5 py-1 font-mono text-[11px] text-violet-200 outline-none placeholder:text-zinc-600 focus:border-violet-500/50"
      />
      <button
        type="button"
        onClick={onToggleReveal}
        title={revealed ? s.hideKey : s.showKey}
        aria-label={revealed ? s.hideKey : s.showKey}
        className="shrink-0 rounded p-1 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200"
      >
        {revealed ? <EyeOff className="size-3.5" /> : <Eye className="size-3.5" />}
      </button>
      {!isPrimary && value.trim() !== "" && (
        <button
          type="button"
          onClick={onMakePrimary}
          title={s.setPrimaryTitle}
          className="shrink-0 rounded px-1.5 py-0.5 text-[11px] text-emerald-300 transition hover:bg-emerald-500/10"
        >
          {s.primaryAction}
        </button>
      )}
      {canRemove && (
        <button
          type="button"
          onClick={onRemove}
          title={s.removeKeyAria}
          aria-label={s.removeKeyAria}
          className="shrink-0 rounded p-1 text-zinc-500 transition hover:bg-red-500/10 hover:text-red-300"
        >
          <Trash2 className="size-3.5" />
        </button>
      )}
    </div>
  );
}

export interface DraftKeyRow {
  value: string;
  tag: string;
  revealed: boolean;
}

/** Dynamic key-row list state: add, edit, reveal, remove, pick primary. */
export function useKeyRows() {
  const [rows, setRows] = useState<DraftKeyRow[]>([{ value: "", tag: "", revealed: false }]);
  const [primaryIdx, setPrimaryIdx] = useState(0);

  function patchRow(index: number, patch: Partial<DraftKeyRow>): void {
    setRows((current) =>
      current.map((row, position) => (position === index ? { ...row, ...patch } : row)),
    );
  }

  function removeRow(index: number): void {
    setRows((current) => {
      if (current.length <= 1) return [{ value: "", tag: "", revealed: false }];
      const next = current.filter((_, position) => position !== index);
      if (primaryIdx >= next.length) setPrimaryIdx(0);
      else if (primaryIdx > index) setPrimaryIdx(primaryIdx - 1);
      return next;
    });
  }

  function resetRows(): void {
    setRows([{ value: "", tag: "", revealed: false }]);
    setPrimaryIdx(0);
  }

  return {
    rows,
    primaryIdx,
    setPrimaryIdx,
    patchRow,
    removeRow,
    resetRows,
    addRow: () => setRows((current) => [...current, { value: "", tag: "", revealed: false }]),
  };
}
