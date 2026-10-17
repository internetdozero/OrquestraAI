import { useEffect, useMemo, useState } from "react";
import { Loader2, Plus, Save, X } from "lucide-react";
import { useI18n } from "../hooks/useI18n";
import {
  formatEnv,
  parseEnv,
  slugify,
  type McpServer,
} from "../lib/mcps";
import { cn } from "../lib/utils";

interface McpFormModalProps {
  /** `null` opens the modal in create mode. */
  server: McpServer | null;
  /** Ids already in the catalog, so a new server cannot silently overwrite one. */
  existingIds: string[];
  saving: boolean;
  error: string | null;
  onClose: () => void;
  onSave: (server: McpServer) => void;
}

const inputClass =
  "h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-2.5 font-mono text-[12px] text-zinc-200 placeholder:text-zinc-600 outline-none transition focus:border-emerald-500/40 focus:bg-zinc-900 focus:ring-2 focus:ring-emerald-500/10";
const labelClass =
  "mb-1 block text-[10px] font-medium tracking-[0.12em] text-zinc-500 uppercase";

export function McpFormModal({
  server,
  existingIds,
  saving,
  error,
  onClose,
  onSave,
}: McpFormModalProps) {
  const { t } = useI18n();
  const creating = server === null;

  const [name, setName] = useState(server?.name ?? "");
  const [id, setId] = useState(server?.id ?? "");
  const [idTouched, setIdTouched] = useState(!creating);
  const [description, setDescription] = useState(server?.description ?? "");
  const [command, setCommand] = useState(server?.command ?? "");
  const [args, setArgs] = useState((server?.args ?? []).join("\n"));
  const [env, setEnv] = useState(formatEnv(server?.env ?? {}));
  const [globalEnabled, setGlobalEnabled] = useState(
    server?.globalEnabled ?? false,
  );
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) onClose();
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, saving]);

  // Follow the display name until the operator edits the id themselves.
  const derivedId = useMemo(() => slugify(name), [name]);
  const effectiveId = idTouched ? id : derivedId;
  const idTaken = creating && existingIds.includes(effectiveId);

  const submit = () => {
    if (saving) return;

    const trimmedName = name.trim();
    const trimmedCommand = command.trim();

    if (!trimmedName) return setLocalError(t.mcps.nameRequired);
    if (!effectiveId) return setLocalError(t.mcps.idRequired);
    if (!/^[a-z0-9][a-z0-9._-]*$/.test(effectiveId)) {
      return setLocalError(t.mcps.idCharset);
    }
    if (idTaken) return setLocalError(t.mcps.idTaken(effectiveId));
    if (!trimmedCommand) return setLocalError(t.mcps.commandRequired);

    setLocalError(null);
    onSave({
      id: effectiveId,
      name: trimmedName,
      description: description.trim(),
      command: trimmedCommand,
      args: args
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean),
      env: parseEnv(env),
      globalEnabled,
      enabledHarnesses: server?.enabledHarnesses ?? [],
    });
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="mcp-form-title"
      className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/70 px-4 py-10 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !saving) onClose();
      }}
    >
      <div className="w-full max-w-2xl overflow-hidden rounded-xl border border-white/[0.08] bg-[#0d0d10] shadow-[0_30px_80px_-30px_rgba(0,0,0,0.9)]">
        <div className="flex items-start justify-between gap-4 border-b border-white/[0.06] px-5 py-4">
          <div>
            <h2
              id="mcp-form-title"
              className="text-sm font-semibold tracking-tight text-zinc-100"
            >
              {creating ? t.mcps.addServer : t.mcps.editTitle(server.name)}
            </h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">
              {t.mcps.formBlurb}
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            disabled={saving}
            aria-label={t.common.close}
            className="grid size-8 shrink-0 place-items-center rounded-lg border border-white/[0.07] text-zinc-500 transition hover:text-zinc-200 disabled:opacity-50"
          >
            <X className="size-4" />
          </button>
        </div>

        <div className="max-h-[62vh] space-y-3.5 overflow-y-auto px-5 py-4">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div>
              <label className={labelClass} htmlFor="mcp-name">
                {t.mcps.fieldName}
              </label>
              <input
                id="mcp-name"
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="GitHub"
                spellCheck={false}
                className={inputClass}
              />
            </div>
            <div>
              <label className={labelClass} htmlFor="mcp-id">
                {t.mcps.fieldId}
              </label>
              <input
                id="mcp-id"
                value={effectiveId}
                onChange={(event) => {
                  setIdTouched(true);
                  setId(event.target.value);
                }}
                placeholder="github"
                spellCheck={false}
                className={cn(
                  inputClass,
                  idTaken && "border-amber-500/50 focus:ring-amber-500/15",
                )}
              />
            </div>
          </div>

          <div>
            <label className={labelClass} htmlFor="mcp-description">
              {t.mcps.fieldDescription}
            </label>
            <input
              id="mcp-description"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              placeholder={t.mcps.descriptionPlaceholder}
              className={inputClass}
            />
          </div>

          <div>
            <label className={labelClass} htmlFor="mcp-command">
              {t.mcps.command}
            </label>
            <input
              id="mcp-command"
              value={command}
              onChange={(event) => setCommand(event.target.value)}
              placeholder="npx"
              spellCheck={false}
              className={inputClass}
            />
          </div>

          <div>
            <label className={labelClass} htmlFor="mcp-args">
              {t.mcps.arguments}{" "}
              <span className="text-zinc-600">· {t.mcps.argsHint}</span>
            </label>
            <textarea
              id="mcp-args"
              value={args}
              onChange={(event) => setArgs(event.target.value)}
              rows={4}
              spellCheck={false}
              placeholder={"-y\n@modelcontextprotocol/server-github"}
              className={cn(inputClass, "h-auto resize-y py-2 leading-relaxed")}
            />
          </div>

          <div>
            <label className={labelClass} htmlFor="mcp-env">
              {t.mcps.environment}{" "}
              <span className="text-zinc-600">· {t.mcps.envHint}</span>
            </label>
            <textarea
              id="mcp-env"
              value={env}
              onChange={(event) => setEnv(event.target.value)}
              rows={3}
              spellCheck={false}
              placeholder="GITHUB_PERSONAL_ACCESS_TOKEN=…"
              className={cn(inputClass, "h-auto resize-y py-2 leading-relaxed")}
            />
            <p className="mt-1 text-[10.5px] leading-relaxed text-zinc-600">
              {t.mcps.envNote}
            </p>
          </div>

          <label className="flex items-center gap-2.5 rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2.5">
            <input
              type="checkbox"
              checked={globalEnabled}
              onChange={(event) => setGlobalEnabled(event.target.checked)}
              className="size-3.5 accent-emerald-500"
            />
            <span className="text-[12px] text-zinc-300">
              {t.mcps.enableGlobally}
              <span className="ml-1.5 text-[11px] text-zinc-500">
                {t.mcps.enableGloballyHint}
              </span>
            </span>
          </label>

          {localError && (
            <p className="rounded-lg border border-amber-500/25 bg-amber-500/[0.07] px-3 py-2 text-[11.5px] text-amber-200">
              {localError}
            </p>
          )}
          {error && (
            <p className="rounded-lg border border-red-500/25 bg-red-500/[0.07] px-3 py-2 text-[11.5px] text-red-200">
              {error}
            </p>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 border-t border-white/[0.06] px-5 py-3">
          <button
            type="button"
            onClick={onClose}
            disabled={saving}
            className="rounded-lg border border-white/10 px-3 py-1.5 text-[12px] font-medium text-zinc-400 transition hover:text-zinc-200 disabled:opacity-50"
          >
            {t.presets.cancel}
          </button>
          <button
            type="button"
            onClick={submit}
            disabled={saving}
            className="flex items-center gap-1.5 rounded-lg border border-emerald-500/30 bg-emerald-500/15 px-3 py-1.5 text-[12px] font-medium text-emerald-200 transition hover:bg-emerald-500/25 disabled:cursor-wait disabled:opacity-60"
          >
            {saving ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : creating ? (
              <Plus className="size-3.5" />
            ) : (
              <Save className="size-3.5" />
            )}
            {creating ? t.mcps.addServerAction : t.mcps.saveChanges}
          </button>
        </div>
      </div>
    </div>
  );
}
