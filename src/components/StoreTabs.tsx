import { useI18n } from "../hooks/useI18n";
import { cn } from "../lib/utils";

/** Segmented Library | Official Store switch shared by both registry views. */
export function StoreTabs({
  showStore,
  onChange,
  className,
}: {
  showStore: boolean;
  onChange: (show: boolean) => void;
  /** Override the outer spacing when embedded in a shared control row. */
  className?: string;
}) {
  const { t } = useI18n();
  const base = "rounded-md px-2.5 py-1 transition";

  return (
    <div className={cn("mb-3 flex w-fit items-center gap-1 rounded-lg border border-white/[0.07] bg-black/30 p-1 text-[11.5px] font-medium", className)}>
      <button
        type="button"
        onClick={() => onChange(false)}
        aria-pressed={!showStore}
        className={`${base} ${showStore ? "text-zinc-500 hover:text-zinc-300" : "bg-white/[0.07] text-zinc-100"}`}
      >
        {t.store.libraryTab}
      </button>
      <button
        type="button"
        onClick={() => onChange(true)}
        aria-pressed={showStore}
        className={`${base} ${showStore ? "bg-amber-500/15 text-amber-200" : "text-zinc-500 hover:text-zinc-300"}`}
      >
        {t.store.storeTab}
      </button>
    </div>
  );
}
