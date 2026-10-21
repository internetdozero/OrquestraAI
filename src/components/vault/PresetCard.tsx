import { ExternalLink } from "lucide-react";
import type { VaultPreset } from "../../lib/vault.ts";
import { cn } from "../../lib/utils.ts";
import { useI18n } from "../../hooks/useI18n.ts";
import { BrandIcon } from "./BrandIcons.tsx";

interface PresetCardProps {
  preset: VaultPreset;
  selected: boolean;
  onSelect: () => void;
}

export function PresetCard({ preset, selected, onSelect }: PresetCardProps) {
  const { t } = useI18n();
  const s = t.credentials;
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      className={cn(
        "group flex items-center gap-2.5 rounded-xl border p-2.5 text-left transition-[border-color,background-color,box-shadow]",
        selected
          ? "border-emerald-500/60 bg-emerald-500/[0.08] shadow-[0_0_18px_-4px_rgba(16,185,129,0.5)]"
          : "border-white/[0.08] bg-white/[0.02] hover:border-white/[0.18] hover:bg-white/[0.05]",
      )}
    >
      <span className="shrink-0 overflow-hidden rounded-lg">
        <BrandIcon presetId={preset.id} size={34} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[12.5px] font-semibold text-zinc-100">
          {preset.label}
        </span>
        <span className="block truncate text-[10.5px] text-zinc-500">
          {preset.description}
        </span>
      </span>
      {preset.keyUrl !== "" && (
        <a
          href={preset.keyUrl}
          target="_blank"
          rel="noreferrer"
          onClick={(event) => event.stopPropagation()}
          title={s.getKeyTitle(preset.keyUrl)}
          aria-label={s.getKeyAria(preset.label)}
          className="shrink-0 rounded p-1 text-zinc-500 transition hover:text-emerald-300"
        >
          <ExternalLink className="size-3.5" />
        </a>
      )}
    </button>
  );
}
