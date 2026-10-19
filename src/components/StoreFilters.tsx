import { cn } from "../lib/utils";

export type StoreTierFilter = "all" | "official" | "community";

interface StoreFiltersProps {
  tier: StoreTierFilter;
  onTierChange: (tier: StoreTierFilter) => void;
  categories: string[];
  category: string;
  onCategoryChange: (category: string) => void;
  labels: {
    all: string;
    official: string;
    community: string;
  };
}

/** Tier + category chips for the Community Hub. Pure presentational. */
export function StoreFilters({
  tier,
  onTierChange,
  categories,
  category,
  onCategoryChange,
  labels,
}: StoreFiltersProps) {
  const tierChip = (id: StoreTierFilter, label: string) => (
    <button
      key={id}
      type="button"
      onClick={() => onTierChange(id)}
      aria-pressed={tier === id}
      className={cn(
        "rounded-full border px-2.5 py-1 text-[11px] font-medium transition",
        tier === id
          ? "border-amber-400/50 bg-amber-400/15 text-amber-100"
          : "border-white/10 bg-white/[0.03] text-zinc-400 hover:text-zinc-200",
      )}
    >
      {label}
    </button>
  );

  return (
    <div className="flex flex-wrap items-center gap-1.5 px-4 py-2.5">
      {tierChip("all", labels.all)}
      {tierChip("official", labels.official)}
      {tierChip("community", labels.community)}
      {categories.length > 0 && (
        <span className="mx-1 h-4 w-px bg-white/10" aria-hidden="true" />
      )}
      {categories.map((entry) => (
        <button
          key={entry}
          type="button"
          onClick={() => onCategoryChange(entry)}
          aria-pressed={category === entry}
          className={cn(
            "rounded-full border px-2.5 py-1 font-mono text-[10.5px] transition",
            category === entry
              ? "border-sky-400/50 bg-sky-400/10 text-sky-100"
              : "border-white/10 bg-transparent text-zinc-500 hover:text-zinc-300",
          )}
        >
          {entry}
        </button>
      ))}
    </div>
  );
}
