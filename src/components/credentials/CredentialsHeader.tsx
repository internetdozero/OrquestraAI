import {
  AlertTriangle,
  KeyRound,
  Layers,
  LayoutGrid,
  List,
  Plus,
  Search,
  ShieldCheck,
  type LucideIcon,
} from "lucide-react";
import { cn } from "../../lib/utils.ts";
import { useI18n } from "../../hooks/useI18n.ts";
import { CustomDropdown } from "../CustomDropdown.tsx";

export type CredentialStatusFilter = "All" | "Healthy" | "RateLimited" | "QuotaExceeded";

export type CredentialViewMode = "grid" | "list";

const STATUS_FILTERS: CredentialStatusFilter[] = ["All", "Healthy", "RateLimited", "QuotaExceeded"];

interface Metric {
  label: string;
  value: number;
  tint: string;
  icon: LucideIcon;
  iconTint: string;
}

interface CredentialsHeaderProps {
  totalServices: number;
  totalKeys: number;
  healthyServices: number;
  degradedServices: number;
  query: string;
  onQueryChange: (query: string) => void;
  statusFilter: CredentialStatusFilter;
  onStatusFilterChange: (filter: CredentialStatusFilter) => void;
  typeOptions: { id: string; label: string }[];
  typeFilter: string;
  onTypeFilterChange: (preset: string) => void;
  view: CredentialViewMode;
  onViewChange: (view: CredentialViewMode) => void;
  onAdd: () => void;
  /** Purge `.migrate-bak` plaintext backups. Absent in browser preview. */
  onPurgeBackups?: () => void;
}

export function CredentialsHeader({
  totalServices,
  totalKeys,
  healthyServices,
  degradedServices,
  query,
  onQueryChange,
  statusFilter,
  onStatusFilterChange,
  typeOptions,
  typeFilter,
  onTypeFilterChange,
  view,
  onViewChange,
  onAdd,
  onPurgeBackups,
}: CredentialsHeaderProps) {
  const { t } = useI18n();
  const s = t.credentials;
  const metrics: Metric[] = [
    {
      label: s.services,
      value: totalServices,
      tint: "bg-gradient-to-b from-zinc-100 to-zinc-400 bg-clip-text text-transparent",
      icon: KeyRound,
      iconTint: "text-zinc-400",
    },
    {
      label: s.pooledKeys,
      value: totalKeys,
      tint: "bg-gradient-to-b from-zinc-100 to-zinc-400 bg-clip-text text-transparent",
      icon: Layers,
      iconTint: "text-zinc-400",
    },
    {
      label: s.healthy,
      value: healthyServices,
      tint: "bg-gradient-to-b from-emerald-200 to-emerald-500 bg-clip-text text-transparent",
      icon: ShieldCheck,
      iconTint: "text-emerald-400",
    },
    {
      label: s.degraded,
      value: degradedServices,
      tint:
        degradedServices > 0
          ? "bg-gradient-to-b from-orange-200 to-orange-500 bg-clip-text text-transparent"
          : "text-zinc-500",
      icon: AlertTriangle,
      iconTint: degradedServices > 0 ? "text-orange-400" : "text-zinc-600",
    },
  ];

  return (
    <div>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {metrics.map((metric) => (
          <div
            key={metric.label}
            className="rounded-xl border border-white/[0.08] bg-white/[0.03] px-3.5 py-2.5 backdrop-blur transition-colors hover:border-white/20"
          >
            <div className="flex items-center justify-between">
              <p className={cn("font-mono text-xl tabular-nums", metric.tint)}>{metric.value}</p>
              <metric.icon className={cn("size-4", metric.iconTint)} />
            </div>
            <p className="mt-0.5 text-[10px] font-medium tracking-[0.12em] text-zinc-500 uppercase">
              {metric.label}
            </p>
          </div>
        ))}
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-2">
        <div className="relative min-w-44 flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-zinc-500" />
          <input
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder={s.searchPlaceholder}
            aria-label={s.searchAria}
            className="w-full rounded-lg border border-white/10 bg-zinc-900 py-1.5 pr-2 pl-8 text-[12.5px] text-zinc-100 placeholder:text-zinc-600"
          />
        </div>
        <CustomDropdown
          value={typeFilter}
          ariaLabel={s.typeAria}
          title={s.typeAria}
          options={typeOptions}
          onChange={onTypeFilterChange}
        />
        <div className="flex gap-1" role="group" aria-label="View mode">
          <button
            type="button"
            onClick={() => onViewChange("grid")}
            title={s.gridAria}
            aria-label={s.gridAria}
            aria-pressed={view === "grid"}
            className={cn(
              "rounded-lg p-2 transition",
              view === "grid"
                ? "accent-bg-soft accent-text"
                : "text-zinc-400 hover:bg-white/[0.05] hover:text-zinc-200",
            )}
          >
            <LayoutGrid className="size-3.5" />
          </button>
          <button
            type="button"
            onClick={() => onViewChange("list")}
            title={s.listAria}
            aria-label={s.listAria}
            aria-pressed={view === "list"}
            className={cn(
              "rounded-lg p-2 transition",
              view === "list"
                ? "accent-bg-soft accent-text"
                : "text-zinc-400 hover:bg-white/[0.05] hover:text-zinc-200",
            )}
          >
            <List className="size-3.5" />
          </button>
        </div>
        <div className="flex gap-1" role="group" aria-label={s.filterHealthAria}>
          {STATUS_FILTERS.map((filter) => (
            <button
              key={filter}
              type="button"
              onClick={() => onStatusFilterChange(filter)}
              className={cn(
                "rounded-lg px-2.5 py-1.5 text-[12px] transition",
                statusFilter === filter
                  ? "accent-bg-soft accent-text"
                  : "text-zinc-400 hover:bg-white/[0.05] hover:text-zinc-200",
              )}
            >
              {s.statusNames[filter]}
            </button>
          ))}
        </div>
        {onPurgeBackups != null && (
          <button
            type="button"
            onClick={onPurgeBackups}
            title={s.purgeBackupsTitle}
            className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[12.5px] font-medium text-zinc-300 transition hover:border-rose-500/40 hover:text-rose-200"
          >
            <ShieldCheck className="size-3.5" /> {s.purgeBackups}
          </button>
        )}
        <button
          type="button"
          onClick={onAdd}
          className="flex items-center gap-1.5 rounded-lg bg-emerald-600 px-3 py-1.5 text-[12.5px] font-medium text-white transition hover:bg-emerald-500"
        >
          <Plus className="size-3.5" /> {s.addService}
        </button>
      </div>
    </div>
  );
}
