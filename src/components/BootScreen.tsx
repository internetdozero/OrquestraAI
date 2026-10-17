import { Loader2 } from "lucide-react";
import { useI18n } from "../hooks/useI18n";

/** Full-screen hold while the first scope and the presets are read. */
export function BootScreen() {
  const { t } = useI18n();

  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 bg-[#09090b] text-zinc-100">
      <Loader2 className="size-5 animate-spin text-emerald-400" />
      <p className="text-sm font-medium">{t.common.loadingResources}</p>
      <p className="max-w-sm px-6 text-center text-[12px] leading-relaxed text-zinc-500">
        {t.common.loadingResourcesHint}
      </p>
    </div>
  );
}
