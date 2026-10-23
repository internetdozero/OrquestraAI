import { cn } from "../../lib/utils.ts";
import { useI18n } from "../../hooks/useI18n.ts";

/**
 * Operator kill-switch: same switch language as the skill cards. Off means
 * the blind proxy refuses the service, so agents only ever reach services
 * that are on.
 */
export function ServiceEnabledToggle({
  enabled,
  busy,
  label,
  onToggle,
}: {
  enabled: boolean;
  busy: boolean;
  /** Service label for the accessible name. */
  label: string;
  onToggle: () => void;
}) {
  const { t } = useI18n();

  return (
    <button
      type="button"
      role="switch"
      aria-checked={enabled}
      aria-label={t.credentials.toggleEnabled(enabled, label)}
      title={t.credentials.toggleEnabled(enabled, label)}
      disabled={busy}
      onClick={onToggle}
      className={cn(
        "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors duration-200 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950 focus-visible:outline-none",
        enabled
          ? "border-emerald-500/50 bg-emerald-600"
          : "border-white/10 bg-zinc-800 hover:bg-zinc-700",
        busy && "cursor-wait opacity-60",
      )}
    >
      <span
        className={cn(
          "block size-3.5 rounded-full bg-white shadow-sm transition-transform duration-200",
          enabled ? "translate-x-[18px]" : "translate-x-[3px]",
        )}
      />
    </button>
  );
}
