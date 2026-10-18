import { ShieldCheck } from "lucide-react";
import { useI18n } from "../hooks/useI18n";

/** Amber verified mark for the official tier (skills + MCP servers). */
export function OfficialBadge() {
  const { t } = useI18n();

  return (
    <span className="flex shrink-0 items-center gap-1 rounded border border-amber-500/25 bg-amber-500/10 px-1.5 py-px text-[9.5px] font-semibold tracking-wide text-amber-300 uppercase">
      <ShieldCheck className="size-2.5" />
      {t.store.official}
    </span>
  );
}
