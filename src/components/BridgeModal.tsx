import { useEffect } from "react";
import {
  AlertTriangle,
  HardDriveDownload,
  Link2,
  Loader2,
  ShieldCheck,
  X,
} from "lucide-react";
import { bridgeNote, bridgeRationale } from "../i18n/bridgeCopy";
import { useI18n } from "../hooks/useI18n";
import type { TranslationSchema } from "../i18n/locales/pt";
import type { HarnessStatus, SecondaryAction } from "../lib/bridge";
import { shortenPath } from "../lib/bridge";
import { cn } from "../lib/utils";

/** Short label for how a secondary discovery location was neutralised. */
function secondaryActionLabel(
  t: TranslationSchema,
  action: SecondaryAction,
  cleared: boolean,
): string {
  if (!cleared) return t.bridge.leaking;
  if (action === "project") return t.bridge.projected;
  if (action === "quarantine") return t.bridge.quarantined;
  return t.bridge.archived;
}

interface BridgeModalProps {
  statuses: HarnessStatus[];
  deploying: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

const pillBase =
  "shrink-0 rounded border px-1.5 py-0.5 font-mono text-[9.5px] font-medium tracking-wide uppercase";

function StatePill({ status }: { status: HarnessStatus }) {
  const { t } = useI18n();
  if (!status.installed) {
    return (
      <span className={cn(pillBase, "border-white/[0.07] bg-white/[0.03] text-zinc-500")}>
        {t.bridge.notInstalled}
      </span>
    );
  }
  if (!status.supported) {
    return (
      <span className={cn(pillBase, "border-amber-500/25 bg-amber-500/[0.08] text-amber-300")}>
        {t.bridge.manual}
      </span>
    );
  }
  if (status.bridged) {
    return (
      <span
        className={cn(pillBase, "border-emerald-500/25 bg-emerald-500/[0.08] text-emerald-300")}
      >
        {t.bridge.linked}
      </span>
    );
  }
  if (!status.secondaryCleared) {
    return (
      <span className={cn(pillBase, "border-rose-500/25 bg-rose-500/[0.08] text-rose-300")}>
        {t.bridge.leakingSkills}
      </span>
    );
  }
  return (
    <span className={cn(pillBase, "border-amber-500/25 bg-amber-500/[0.08] text-amber-300")}>
      {t.bridge.unlinked}
    </span>
  );
}

function Route({ from, to }: { from: string; to: string }) {
  return (
    <p className="flex flex-wrap items-center gap-1.5 font-mono text-[10px] leading-relaxed text-zinc-500">
      <span className="text-zinc-400">{shortenPath(from)}</span>
      <span className="text-zinc-700">→</span>
      <span className="text-zinc-400">{to}</span>
    </p>
  );
}

function HarnessRow({ status }: { status: HarnessStatus }) {
  const { t } = useI18n();
  const projection = `~/.orquestra/active/${status.id}`;
  const master = "~/.orquestra/AGENTS.md";

  return (
    <li className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <Link2 className="size-3 shrink-0 text-zinc-600" />
          <span className="truncate text-[12.5px] font-medium text-zinc-200">
            {status.label}
          </span>
        </div>
        <StatePill status={status} />
      </div>

      {status.installed ? (
        <div className="mt-1.5 space-y-1 pl-5">
          {status.skillsPaths.map((path) => (
            <Route key={path} from={path} to={projection} />
          ))}
          {status.instructionsPath && (
            <Route from={status.instructionsPath} to={master} />
          )}
          {status.configPath && (
            <p className="font-mono text-[10px] leading-relaxed text-zinc-500">
              <span className="text-zinc-400">{shortenPath(status.configPath)}</span>
              <span className="text-zinc-700"> · </span>
              skills.paths → {projection}
            </p>
          )}
          {status.secondaryPaths.map((entry) => (
            <p
              key={entry.path}
              className="flex flex-wrap items-center gap-1.5 font-mono text-[10px] leading-relaxed"
            >
              <span
                className={cn(
                  "shrink-0 rounded border px-1 py-px text-[9px] tracking-wide uppercase",
                  entry.cleared
                    ? "border-emerald-500/25 bg-emerald-500/[0.08] text-emerald-300"
                    : "border-rose-500/25 bg-rose-500/[0.08] text-rose-300",
                )}
              >
                {secondaryActionLabel(t, entry.action, entry.cleared)}
              </span>
              <span className="text-zinc-400">{shortenPath(entry.path)}</span>
              <span className="text-zinc-700">·</span>
              <span className="text-zinc-500">{bridgeRationale(t, entry.rationale)}</span>
            </p>
          ))}
          {status.note && (
            <p className="text-[10.5px] leading-relaxed text-zinc-600">
              {bridgeNote(t, status.note)}
            </p>
          )}
        </div>
      ) : (
        <p className="mt-1.5 pl-5 text-[10.5px] text-zinc-600">
          {t.bridge.untouched}
        </p>
      )}
    </li>
  );
}

export function BridgeModal({
  statuses,
  deploying,
  onClose,
  onConfirm,
}: BridgeModalProps) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !deploying) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, deploying]);

  const { t } = useI18n();
  const managed = statuses.filter((status) => status.installed && status.supported);
  const linked = managed.filter((status) => status.bridged).length;
  const pending = managed.length - linked;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !deploying) onClose();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="bridge-modal-title"
        className="flex max-h-[84vh] w-full max-w-2xl flex-col overflow-hidden rounded-2xl border border-white/10 bg-[#0c0c10] shadow-[0_30px_80px_-20px_rgba(0,0,0,0.9)]"
      >
        <header className="flex items-start justify-between gap-4 border-b border-white/[0.06] px-5 py-4">
          <div className="flex items-start gap-3">
            <div className="grid size-9 shrink-0 place-items-center rounded-lg border border-emerald-500/25 bg-emerald-500/10 text-emerald-300">
              <ShieldCheck className="size-4" />
            </div>
            <div>
              <h2
                id="bridge-modal-title"
                className="text-sm font-semibold tracking-tight text-zinc-100"
              >
                {t.bridge.title}
              </h2>
              <p className="mt-0.5 text-[11px] leading-relaxed text-zinc-500">
                {t.bridge.summary(managed.length, linked, pending)}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            disabled={deploying}
            aria-label={t.common.close}
            className="rounded-md p-1 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200 disabled:opacity-40"
          >
            <X className="size-4" />
          </button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
          <div className="mb-4 rounded-lg border border-amber-500/20 bg-amber-500/[0.05] p-3">
            <div className="flex items-start gap-2">
              <AlertTriangle className="mt-px size-3.5 shrink-0 text-amber-400" />
              <div className="space-y-1 text-[11px] leading-relaxed text-amber-100/80">
                <p>
                  {t.bridge.warnLead}{" "}
                  <code className="font-mono text-amber-200">
                    ~/.orquestra/backups/pre-fusion-&lt;timestamp&gt;/
                  </code>{" "}
                  {t.bridge.warnTail}
                </p>
                <p className="text-amber-100/60">{t.bridge.warnSees}</p>
                <p className="text-amber-100/60">{t.bridge.warnSecondary}</p>
              </div>
            </div>
          </div>

          <div className="mb-2 flex items-center justify-between">
            <span className="text-[10px] font-medium tracking-[0.14em] text-zinc-500 uppercase">
              {t.bridge.linkPlan}
            </span>
            <span className="font-mono text-[10px] text-zinc-600">
              {t.bridge.masterLabel}
            </span>
          </div>

          <ul className="space-y-2">
            {statuses.map((status) => (
              <HarnessRow key={status.id} status={status} />
            ))}
          </ul>
        </div>

        <footer className="flex items-center justify-between gap-3 border-t border-white/[0.06] bg-white/[0.015] px-5 py-3">
          <span className="flex items-center gap-1.5 text-[10.5px] text-zinc-600">
            <HardDriveDownload className="size-3" />
            {t.bridge.reversible}
          </span>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={onClose}
              disabled={deploying}
              className="rounded-md border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11.5px] font-medium text-zinc-300 transition hover:border-white/20 hover:text-white disabled:opacity-50"
            >
              {t.presets.cancel}
            </button>
            <button
              type="button"
              onClick={onConfirm}
              disabled={deploying || managed.length === 0}
              className="flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/15 px-3 py-1.5 text-[11.5px] font-semibold text-emerald-200 transition hover:border-emerald-400/50 hover:bg-emerald-500/25 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {deploying ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <Link2 className="size-3.5" />
              )}
              {deploying ? t.bridge.linkingHarnesses : t.bridge.backupConnect}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
