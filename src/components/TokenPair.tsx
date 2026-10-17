import { Cpu } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import { formatTokens } from "../lib/utils";

/** Estimated prompt weight: the listing every turn, and the full body on use. */
export function TokenPair({
  entry,
  full,
  fullHint,
}: {
  entry: number;
  full: number;
  fullHint: string;
}) {
  const { t } = useI18n();

  return (
    <span className="flex shrink-0 items-center gap-1.5 font-mono text-[10.5px] whitespace-nowrap text-zinc-500">
      <Cpu className="size-3 shrink-0 text-zinc-600" />
      <span title={t.tokens.entryHint}>
        {t.tokens.entry} ≈ {formatTokens(entry)}
      </span>
      <span className="text-zinc-700">·</span>
      <span title={fullHint}>
        {t.tokens.full} ≈ {formatTokens(full)} {t.tokens.unit}
      </span>
    </span>
  );
}
