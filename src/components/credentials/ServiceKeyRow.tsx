import { useState } from "react";
import { Check, Crown, Loader2, Pencil, PlugZap, RefreshCw, Trash2, X } from "lucide-react";
import type { VaultKeyHealth, VaultTestResult } from "../../lib/vault.ts";
import { cn } from "../../lib/utils.ts";
import { useI18n } from "../../hooks/useI18n.ts";

/** Badge tint shared by the card and the header filters. */
export function healthBadgeClass(health: string): string {
  switch (health) {
    case "Healthy":
    case "healthy":
      return "bg-emerald-500/15 text-emerald-300 border-emerald-500/30";
    case "Degraded":
      return "bg-amber-500/15 text-amber-300 border-amber-500/30";
    case "RateLimited":
    case "rateLimited":
      return "bg-orange-500/15 text-orange-300 border-orange-500/30";
    case "QuotaExceeded":
    case "quotaExceeded":
    case "InvalidKey":
    case "invalidKey":
      return "bg-red-500/15 text-red-300 border-red-500/30";
    default:
      return "bg-zinc-500/15 text-zinc-300 border-zinc-500/30";
  }
}

/** Human cooldown remainder from an epoch-ms expiry (0 = none).
 * Callers pass translated formatters so PT shows "espera ~…" and EN shows
 * "cooldown ~…". */
export function formatCooldownWith(
  cooldownUntilMs: number,
  minutesLabel: (minutes: number) => string,
  hoursLabel: (hours: number, minutes: number) => string,
): string | null {
  const remaining = cooldownUntilMs - Date.now();
  if (remaining <= 0) return null;
  const minutes = Math.ceil(remaining / 60000);
  return minutes >= 60
    ? hoursLabel(Math.floor(minutes / 60), minutes % 60)
    : minutesLabel(minutes);
}

interface ServiceKeyRowProps {
  index: number;
  masked: string;
  tag?: string | null;
  keyHealth: VaultKeyHealth;
  cooldownUntilMs: number;
  isPrimary: boolean;
  canRemove: boolean;
  onPromote: () => Promise<void>;
  onRemove: () => Promise<void>;
  onTest: () => Promise<VaultTestResult>;
  onSaveTag: (tag: string | null) => Promise<void>;
  onReplaceKey: (secret: string) => Promise<void>;
}

export function ServiceKeyRow({
  index,
  masked,
  tag,
  keyHealth,
  cooldownUntilMs,
  isPrimary,
  canRemove,
  onPromote,
  onRemove,
  onTest,
  onSaveTag,
  onReplaceKey,
}: ServiceKeyRowProps) {
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<VaultTestResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [editingTag, setEditingTag] = useState(false);
  const [tagDraft, setTagDraft] = useState(tag ?? "");
  const [replacing, setReplacing] = useState(false);
  const [secretDraft, setSecretDraft] = useState("");
  const { t } = useI18n();
  const s = t.credentials;
  const cooldown = formatCooldownWith(cooldownUntilMs, s.cooldownMinutes, s.cooldownHours);
  const locked = testing || busy;

  async function runGuarded(action: () => Promise<void>): Promise<void> {
    setBusy(true);
    try {
      await action();
    } finally {
      setBusy(false);
    }
  }

  async function saveTag(): Promise<void> {
    const next = tagDraft.trim() || null;
    setBusy(true);
    try {
      await onSaveTag(next);
      setEditingTag(false);
    } finally {
      setBusy(false);
    }
  }

  function cancelTagEdit(): void {
    setTagDraft(tag ?? "");
    setEditingTag(false);
  }

  async function saveReplacement(): Promise<void> {
    if (secretDraft.trim() === "") return;
    setBusy(true);
    try {
      await onReplaceKey(secretDraft.trim());
      setSecretDraft("");
      setReplacing(false);
    } finally {
      setBusy(false);
    }
  }

  async function handleTest(): Promise<void> {
    setTesting(true);
    try {
      setResult(await onTest());
    } catch (err) {
      setResult({
        success: false,
        latencyMs: 0,
        message: err instanceof Error ? err.message : String(err),
        health: keyHealth,
      });
    } finally {
      setTesting(false);
    }
  }

  return (
    <div
      className={cn(
        "rounded-md border px-2 py-1.5",
        isPrimary
          ? "border-emerald-500/40 bg-emerald-500/[0.06]"
          : "border-white/[0.07] bg-white/[0.02]",
      )}
    >
      <div className="flex items-center gap-2 text-xs">
        {isPrimary ? (
          <Crown className="size-3.5 shrink-0 text-emerald-300" />
        ) : (
          <span className="shrink-0 font-mono text-[10px] text-zinc-500">
            #{index}
          </span>
        )}
        <code className="min-w-0 flex-1 truncate font-mono text-zinc-200">
          {masked}
        </code>
        {editingTag ? (
          <span className="flex shrink-0 items-center gap-1">
            <input
              value={tagDraft}
              onChange={(event) => setTagDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void saveTag();
                if (event.key === "Escape") cancelTagEdit();
              }}
              placeholder={s.newTagPlaceholder}
              aria-label={s.tagAriaBackup(index)}
              autoComplete="off"
              spellCheck={false}
              autoFocus
              className="w-20 rounded border border-violet-500/50 bg-zinc-950 px-1 py-px font-mono text-[10px] text-violet-200 outline-none placeholder:text-zinc-600"
            />
            <button
              type="button"
              onClick={() => void saveTag()}
              disabled={locked}
              title={s.saveTag}
              aria-label={s.saveTag}
              className="rounded p-0.5 text-emerald-300 transition hover:bg-emerald-500/10 disabled:opacity-50"
            >
              <Check className="size-3" />
            </button>
            <button
              type="button"
              onClick={cancelTagEdit}
              title={s.cancel}
              aria-label={s.cancel}
              className="rounded p-0.5 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200"
            >
              <X className="size-3" />
            </button>
          </span>
        ) : (
          <span className="flex shrink-0 items-center gap-0.5">
            {tag !== undefined && tag !== null && tag.trim() !== "" ? (
              <span
                title={tag.trim()}
                className="rounded-full border border-violet-500/40 bg-violet-500/15 px-1.5 py-px font-mono text-[10px] font-medium text-violet-200"
              >
                {tag.trim()}
              </span>
            ) : (
              <span className="rounded-full border border-dashed border-white/15 px-1.5 py-px font-mono text-[10px] text-zinc-600">
                {s.untagged}
              </span>
            )}
            <button
              type="button"
              onClick={() => {
                setTagDraft(tag ?? "");
                setEditingTag(true);
              }}
              disabled={locked || replacing}
              title={tag ? s.renameTag : s.addTag}
              aria-label={tag ? s.renameTag : s.addTag}
              className="rounded p-0.5 text-zinc-600 transition hover:bg-white/[0.06] hover:text-violet-300 disabled:opacity-50"
            >
              <Pencil className="size-3" />
            </button>
          </span>
        )}
        <span
          className={cn(
            "shrink-0 rounded border px-1 py-px text-[10px]",
            healthBadgeClass(keyHealth),
          )}
        >
          {s.healthName(keyHealth)}
        </span>
        <button
          type="button"
          onClick={() => {
            setReplacing((current) => !current);
            setSecretDraft("");
          }}
          disabled={locked}
          title={s.replaceTitle}
          aria-label={s.replaceAria}
          className="shrink-0 rounded p-1 text-zinc-500 transition hover:bg-white/[0.06] hover:text-amber-300 disabled:opacity-50"
        >
          <RefreshCw className="size-3.5" />
        </button>
        <button
          type="button"
          onClick={() => void handleTest()}
          disabled={locked}
          title={s.perKeyTestTitle}
          className="shrink-0 rounded p-1 text-zinc-400 transition hover:bg-white/[0.06] hover:text-zinc-100 disabled:opacity-50"
        >
          {testing ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : (
            <PlugZap className="size-3.5" />
          )}
        </button>
        {!isPrimary && (
          <button
            type="button"
            onClick={() => void runGuarded(onPromote)}
            disabled={testing || busy}
            title={s.promoteTitle}
            className="shrink-0 rounded px-1.5 py-0.5 text-[11px] text-emerald-300 transition hover:bg-emerald-500/10 disabled:opacity-50"
          >
            {s.primaryAction}
          </button>
        )}
        {canRemove && (
          <button
            type="button"
            onClick={() => void runGuarded(onRemove)}
            disabled={testing || busy}
            title={s.removeTitle}
            className="shrink-0 rounded p-1 text-zinc-500 transition hover:bg-red-500/10 hover:text-red-300 disabled:opacity-50"
          >
            <Trash2 className="size-3.5" />
          </button>
        )}
      </div>
      {replacing && (
        <div className="mt-1.5 flex items-center gap-1.5">
          <input
            type="password"
            value={secretDraft}
            onChange={(event) => setSecretDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void saveReplacement();
              if (event.key === "Escape") setReplacing(false);
            }}
            placeholder={s.replacePlaceholder}
            aria-label={s.replaceAria}
            autoComplete="off"
            spellCheck={false}
            autoFocus
            className="min-w-0 flex-1 rounded border border-amber-500/40 bg-zinc-950 px-1.5 py-1 font-mono text-[11px] text-zinc-100 outline-none placeholder:text-zinc-600"
          />
          <button
            type="button"
            onClick={() => void saveReplacement()}
            disabled={locked || secretDraft.trim() === ""}
            className="shrink-0 rounded bg-amber-600 px-2 py-1 text-[11px] font-medium text-white disabled:opacity-50"
          >
            {s.replaceAction}
          </button>
          <button
            type="button"
            onClick={() => setReplacing(false)}
            className="shrink-0 rounded px-1.5 py-1 text-[11px] text-zinc-400 hover:bg-white/[0.06]"
          >
            {s.cancel}
          </button>
        </div>
      )}
      {cooldown !== null && (
        <p className="mt-1 text-[10.5px] text-orange-300/90">{cooldown}</p>
      )}
      {result !== null && (
        <p
          className={cn(
            "mt-1 truncate text-[10.5px]",
            result.success ? "text-emerald-300" : "text-red-300",
          )}
          title={result.message}
        >
          {result.success ? "✓ " : "✗ "}
          {result.message}
        </p>
      )}
    </div>
  );
}
