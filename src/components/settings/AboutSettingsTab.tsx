import { ClipboardCopy, Orbit } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { HARNESSES } from "../../lib/harnesses";
import type { CoreStatus } from "../../lib/tauri";

interface AboutSettingsTabProps {
  status: CoreStatus | null;
  isTauri: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
}

/** Version, platform info and a clipboard diagnostics snippet. */
export function AboutSettingsTab({ status, isTauri, onToast, onError }: AboutSettingsTabProps) {
  const { t } = useI18n();
  const a = t.settings.about;

  const copyDiagnostics = async () => {
    const lines = [
      "# OrquestraAI diagnostics",
      `- version: ${status?.version ?? "0.1.0"}`,
      `- platform: ${status?.platform ?? "browser-preview"}`,
      `- core: ${status?.name ?? "orquestra-ai"}`,
      `- tauri: ${isTauri ? "desktop" : "browser-preview"}`,
      `- harnesses: ${HARNESSES.map((h) => h.id).join(", ")}`,
      `- bridge: ${status?.bridge ?? "unknown"}`,
      `- date: ${new Date().toISOString()}`,
    ];
    try {
      await navigator.clipboard.writeText(lines.join("\n"));
      onToast(a.copied);
    } catch (cause) {
      const detail = cause instanceof Error ? cause.message : String(cause);
      onError(a.copyFailed(detail));
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3 rounded-xl border border-white/[0.07] bg-white/[0.02] p-3.5">
        <div className="grid size-10 shrink-0 place-items-center rounded-xl bg-gradient-to-br from-emerald-400/20 via-emerald-500/10 to-cyan-500/10 ring-1 ring-emerald-400/25">
          <Orbit className="size-5 text-emerald-300" />
        </div>
        <div>
          <p className="text-[13px] font-semibold text-zinc-100">
            Orquestra<span className="text-emerald-400">AI</span>
          </p>
          <p className="font-mono text-[10.5px] text-zinc-500">
            {a.version} {status?.version ?? "0.1.0"} · {a.platform}{" "}
            {status?.platform ?? "browser-preview"}
          </p>
        </div>
      </div>

      <dl className="space-y-1.5">
        {[
          [a.core, status?.name ?? "orquestra-ai"],
          ["Bridge", status?.bridge ?? "—"],
          ["Harnesses", `${HARNESSES.length}`],
        ].map(([term, value]) => (
          <div
            key={term}
            className="flex items-center justify-between rounded-lg border border-white/[0.05] bg-white/[0.015] px-3 py-2"
          >
            <dt className="text-[12px] text-zinc-500">{term}</dt>
            <dd className="font-mono text-[11.5px] text-zinc-200">{value}</dd>
          </div>
        ))}
      </dl>

      <button
        type="button"
        onClick={() => void copyDiagnostics()}
        className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11.5px] font-medium text-zinc-200 transition hover:border-white/20"
      >
        <ClipboardCopy className="size-3.5 text-emerald-400" />
        {a.copyDiag}
      </button>
    </div>
  );
}
