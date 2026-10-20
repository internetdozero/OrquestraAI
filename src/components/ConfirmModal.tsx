import { useEffect } from "react";
import { createPortal } from "react-dom";
import { Info, Loader2, Trash2, TriangleAlert } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import { cn } from "../lib/utils";

export type ConfirmTone = "danger" | "warning" | "neutral";

export interface ConfirmModalProps {
  title: string;
  message: React.ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  tone?: ConfirmTone;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

const TONE_STYLES: Record<
  ConfirmTone,
  { badge: string; icon: string; confirm: string; Icon: typeof Trash2 }
> = {
  danger: {
    badge: "border-rose-500/30 bg-rose-500/10",
    icon: "text-rose-300",
    confirm: "bg-rose-600 hover:bg-rose-500",
    Icon: Trash2,
  },
  warning: {
    badge: "border-amber-500/30 bg-amber-500/10",
    icon: "text-amber-300",
    confirm: "bg-amber-600 hover:bg-amber-500",
    Icon: TriangleAlert,
  },
  neutral: {
    badge: "border-zinc-500/30 bg-zinc-500/10",
    icon: "text-zinc-300",
    confirm: "bg-white/10 hover:bg-white/20",
    Icon: Info,
  },
};

/**
 * App-themed replacement for `window.confirm`: portal-rendered above every
 * drawer/modal (`z-[120]` clears `ArchivedIdeasModal` at `z-[100]`),
 * dismissible via Escape or backdrop click.
 */
export function ConfirmModal({
  title,
  message,
  confirmLabel,
  cancelLabel,
  tone = "danger",
  busy = false,
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  const { t } = useI18n();
  const style = TONE_STYLES[tone];
  const Icon = style.Icon;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onCancel();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onCancel, busy]);

  if (typeof document === "undefined") return null;

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={title}
      className="fixed inset-0 z-[120] grid place-items-center bg-black/70 p-4 backdrop-blur-sm"
      onClick={() => { if (!busy) onCancel(); }}
    >
      <div
        className="w-full max-w-md rounded-2xl border border-white/10 bg-[#101014] p-5 shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-center gap-2.5">
          <div className={cn("grid size-9 place-items-center rounded-xl border", style.badge)}>
            <Icon className={cn("size-4", style.icon)} />
          </div>
          <h2 className="text-sm font-semibold text-zinc-100">{title}</h2>
        </div>

        <div className="mt-4 text-[12.5px] leading-relaxed text-zinc-300">
          {message}
        </div>

        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            disabled={busy}
            className="rounded-lg border border-white/10 bg-white/[0.03] px-3.5 py-2 text-[12px] font-medium text-zinc-300 transition hover:border-white/20 hover:text-white disabled:opacity-50"
          >
            {cancelLabel ?? t.common.dismiss}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={busy}
            className={cn(
              "flex items-center gap-1.5 rounded-lg px-3.5 py-2 text-[12px] font-semibold text-white transition disabled:cursor-wait disabled:opacity-60",
              style.confirm,
            )}
          >
            {busy && <Loader2 className="size-3.5 animate-spin" />}
            {confirmLabel ?? t.common.delete}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
