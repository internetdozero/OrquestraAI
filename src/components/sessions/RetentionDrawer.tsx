import { useState } from "react";
import { createPortal } from "react-dom";
import { ShieldCheck, X } from "lucide-react";
import { HARNESSES } from "../../lib/harnesses";
import { useI18n } from "../../hooks/useI18n";
import type { RetentionPolicy } from "../../lib/sessions";

interface RetentionDrawerProps {
  policy: RetentionPolicy | null;
  onClose: () => void;
  onSave: (policy: RetentionPolicy) => void;
}

/** Slide-over to configure auto-purge TTLs. Pins are always preserved. */
export function RetentionDrawer({ policy, onClose, onSave }: RetentionDrawerProps) {
  const { t } = useI18n();
  const s = t.sessions;
  const [enabled, setEnabled] = useState(policy?.autoPurgeEnabled ?? false);
  const [interval, setInterval] = useState(policy?.autoPurgeIntervalHours ?? 24);
  const [ttl, setTtl] = useState(policy?.defaultTtlDays ?? 15);
  const [overrides, setOverrides] = useState<Record<string, number>>(
    policy?.harnessOverrides ?? {},
  );

  if (typeof document === "undefined") return null;

  // Portaled to the body above every shell layer: inside <main> the fixed
  // backdrop would otherwise paint beneath the header/footer stacking
  // contexts.
  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={s.retentionTitle}
      className="fixed inset-0 z-[100] flex justify-end bg-black/60 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="flex h-full w-full max-w-sm flex-col border-l border-white/10 bg-[#101014] p-5"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <h2 className="flex items-center gap-2 text-sm font-semibold text-zinc-100">
            <ShieldCheck className="size-4 text-emerald-400" />
            {s.retentionTitle}
          </h2>
          <button
            type="button"
            onClick={onClose}
            aria-label={t.common.close}
            className="grid size-7 place-items-center rounded-md text-zinc-500 hover:bg-white/[0.05] hover:text-zinc-200"
          >
            <X className="size-4" />
          </button>
        </div>
        <p className="mt-1 text-[11.5px] text-zinc-500">{s.retentionBody}</p>

        <label className="mt-4 flex cursor-pointer items-center justify-between rounded-lg border border-white/[0.07] bg-white/[0.02] px-3 py-2.5">
          <span className="text-[12.5px] text-zinc-200">{s.enableAuto}</span>
          <button
            type="button"
            role="switch"
            aria-checked={enabled}
            onClick={() => setEnabled((v) => !v)}
            className={`relative h-5.5 w-10 rounded-full transition ${enabled ? "bg-emerald-500" : "bg-zinc-700"}`}
          >
            <span
              className={`absolute top-0.5 size-4.5 rounded-full bg-white transition-[left] ${enabled ? "left-5" : "left-0.5"}`}
            />
          </button>
        </label>

        <div className="mt-3 grid grid-cols-2 gap-2">
          <label className="rounded-lg border border-white/[0.07] bg-white/[0.02] p-2.5">
            <span className="text-[10.5px] text-zinc-500">{s.intervalHours}</span>
            <input
              type="number"
              min={1}
              max={720}
              value={interval}
              onChange={(e) => setInterval(Number(e.target.value) || 24)}
              className="mt-1 w-full bg-transparent font-mono text-sm text-zinc-100 outline-none [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
            />
          </label>
          <label className="rounded-lg border border-white/[0.07] bg-white/[0.02] p-2.5">
            <span className="text-[10.5px] text-zinc-500">{s.defaultTtl}</span>
            <input
              type="number"
              min={1}
              max={365}
              value={ttl}
              onChange={(e) => setTtl(Number(e.target.value) || 15)}
              className="mt-1 w-full bg-transparent font-mono text-sm text-zinc-100 outline-none [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
            />
          </label>
        </div>

        <input
          type="range"
          min={1}
          max={90}
          value={ttl}
          onChange={(e) => setTtl(Number(e.target.value))}
          aria-label={s.defaultTtl}
          className="mt-3 w-full accent-emerald-500"
        />

        <p className="mt-4 text-[10.5px] tracking-wider text-zinc-500 uppercase">{s.overrides}</p>
        <div className="scroll-pane mt-2 flex-1 space-y-1.5 overflow-y-auto pr-1">
          {HARNESSES.map((h) => (
            <label
              key={h.id}
              className="flex items-center justify-between gap-2 rounded-lg border border-white/[0.05] bg-white/[0.015] px-2.5 py-1.5"
            >
              <span className="truncate text-[12px] text-zinc-300">{h.label}</span>
              <span className="flex shrink-0 items-center gap-1">
                <input
                  type="number"
                  min={1}
                  max={365}
                  placeholder={`${ttl}`}
                  value={overrides[h.id] ?? ""}
                  onChange={(e) => {
                    const v = e.target.value;
                    setOverrides((prev) => {
                      if (v === "") {
                        const next = { ...prev };
                        delete next[h.id];
                        return next;
                      }
                      return { ...prev, [h.id]: Number(v) || ttl };
                    });
                  }}
                  aria-label={`${h.label} TTL`}
                  className="w-16 rounded border border-white/10 bg-zinc-900/60 px-2 py-0.5 text-center font-mono text-[11.5px] text-zinc-100 outline-none focus:border-emerald-500/40 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
                />
                <span className="text-[10px] text-zinc-600">{s.perHarness}</span>
              </span>
            </label>
          ))}
        </div>

        <button
          type="button"
          onClick={() =>
            onSave({
              defaultTtlDays: ttl,
              harnessOverrides: overrides,
              autoPurgeEnabled: enabled,
              autoPurgeIntervalHours: interval,
              lastRun: policy?.lastRun ?? null,
            })
          }
          className="mt-4 rounded-lg bg-emerald-600 py-2 text-[12.5px] font-semibold text-white transition hover:bg-emerald-500"
        >
          {s.save}
        </button>
      </div>
    </div>,
    document.body,
  );
}
