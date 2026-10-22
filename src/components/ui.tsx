import { useCallback, type HTMLAttributes, type ReactNode } from "react";
import { AlertTriangle, type LucideIcon } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import { cn } from "../lib/utils";

/** Small bordered button shared by the panel toolbars and empty states. */
export const actionButtonClass =
  "rounded-md border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11px] font-medium whitespace-nowrap text-zinc-300 transition hover:border-white/20 hover:text-white disabled:cursor-not-allowed disabled:opacity-50 focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none";

/** Compact metric tile used by the skills dashboard. */
export function StatTile({
  label,
  value,
  hint,
  icon: Icon,
  accent = false,
}: {
  label: string;
  value: ReactNode;
  hint: string;
  icon: LucideIcon;
  accent?: boolean;
}) {
  return (
    <div
      className={cn(
        "relative overflow-hidden rounded-xl border border-white/[0.06] bg-zinc-900/30 p-3.5",
        accent && "accent-border accent-bg-soft",
      )}
    >
      {accent && (
        <div className="accent-bg-soft pointer-events-none absolute -top-10 -right-10 size-24 rounded-full blur-2xl" />
      )}
      <div className="flex items-center justify-between">
        <span className="truncate text-[10.5px] font-medium tracking-wider text-zinc-500 uppercase">
          {label}
        </span>
        <Icon className={cn("size-3.5 shrink-0", accent ? "accent-text" : "text-zinc-600")} />
      </div>
      <div className="mt-1.5 font-mono text-xl font-semibold tracking-tight text-zinc-100">
        {value}
      </div>
      <p className="mt-0.5 truncate text-[11px] text-zinc-500">{hint}</p>
    </div>
  );
}

/** Dashed container for empty, error and unavailable states. */
export function StatePanel({
  icon: Icon,
  title,
  description,
  children,
}: {
  icon: LucideIcon;
  title: string;
  description: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-white/10 bg-zinc-900/20 px-6 py-16 text-center">
      <div className="grid size-10 place-items-center rounded-full border border-white/[0.07] bg-white/[0.03]">
        <Icon className="size-4 text-zinc-500" />
      </div>
      <div className="max-w-md">
        <p className="text-sm font-medium text-zinc-300">{title}</p>
        <p className="mt-1 text-[11px] leading-relaxed text-zinc-500">{description}</p>
      </div>
      {children}
    </div>
  );
}

/**
 * CSP-safe dynamic height. `style-src` carries no `unsafe-inline`, so
 * runtime pixel values go through CSSOM (script served from 'self',
 * allowed) instead of the style attribute (blocked).
 */
export function SpacerDiv({
  height,
  ...props
}: { height: number } & HTMLAttributes<HTMLDivElement>) {
  const ref = useCallback(
    (element: HTMLDivElement | null) => {
      if (element) element.style.height = `${height}px`;
    },
    [height],
  );
  return <div ref={ref} {...props} />;
}

/** Dismissible inline error banner. */
export function ErrorBanner({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  const { t } = useI18n();

  return (
    <div
      role="alert"
      className="mb-3 flex items-start gap-2.5 rounded-lg border border-rose-500/25 bg-rose-500/[0.06] px-3 py-2.5 text-[12px] text-rose-200"
    >
      <AlertTriangle className="mt-px size-3.5 shrink-0 text-rose-400" />
      <p className="min-w-0 flex-1 leading-relaxed">{message}</p>
      <button
        type="button"
        onClick={onDismiss}
        className="shrink-0 text-[11px] font-medium text-rose-300/80 transition hover:text-rose-100"
      >
        {t.common.dismiss}
      </button>
    </div>
  );
}

/** Placeholder grid shown while the vault is being scanned. */
export function SkeletonGrid() {
  return (
    <div className="grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-3">
      {Array.from({ length: 6 }, (_, index) => (
        <div
          key={index}
          className="animate-pulse rounded-xl border border-white/[0.06] bg-zinc-900/30 p-4"
        >
          <div className="flex items-start gap-3">
            <div className="size-9 rounded-lg bg-white/[0.06]" />
            <div className="flex-1 space-y-2 pt-0.5">
              <div className="h-3 w-2/3 rounded bg-white/[0.06]" />
              <div className="h-2 w-1/3 rounded bg-white/[0.04]" />
            </div>
          </div>
          <div className="mt-4 space-y-2">
            <div className="h-2.5 w-full rounded bg-white/[0.05]" />
            <div className="h-2.5 w-4/5 rounded bg-white/[0.05]" />
          </div>
          <div className="mt-4 h-2.5 w-1/3 rounded bg-white/[0.04]" />
        </div>
      ))}
    </div>
  );
}
