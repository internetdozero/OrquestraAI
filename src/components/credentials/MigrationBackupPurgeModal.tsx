import { createPortal } from "react-dom";
import { FileWarning, TriangleAlert } from "lucide-react";
import { useI18n } from "../../hooks/useI18n.ts";
import { cn } from "../../lib/utils.ts";

interface MigrationBackupPurgeModalProps {
  open: boolean;
  purging: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

/**
 * Explicit user confirmation before the backend zeroizes and unlinks the
 * `.migrate-bak` plaintext backups. The purge only destroys a backup whose
 * sealed file still opens, but it is one-way: the modal says so plainly and
 * points at key rotation for pre-encryption secrets.
 */
export function MigrationBackupPurgeModal({
  open,
  purging,
  onCancel,
  onConfirm,
}: MigrationBackupPurgeModalProps) {
  const { t } = useI18n();
  const s = t.credentials;

  if (!open || typeof document === "undefined") return null;

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={s.purgeBackupsTitle}
      className="fixed inset-0 z-[100] grid place-items-center bg-black/60 p-4 backdrop-blur-sm"
      onClick={onCancel}
    >
      <div
        className="w-full max-w-md rounded-2xl border border-white/10 bg-[#101014] p-5 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2.5">
          <div className="grid size-9 place-items-center rounded-xl border border-rose-500/30 bg-rose-500/10">
            <TriangleAlert className="size-4 text-rose-300" />
          </div>
          <div>
            <h2 className="text-sm font-semibold text-zinc-100">{s.purgeBackupsTitle}</h2>
            <p className="font-mono text-[10.5px] text-zinc-500">*.migrate-bak</p>
          </div>
        </div>

        <p className="mt-4 text-[12.5px] leading-relaxed text-zinc-300">{s.purgeBackupsLead}</p>

        <div className="mt-3 space-y-1.5 rounded-xl border border-white/[0.07] bg-white/[0.02] p-3">
          {["vault.json.migrate-bak", "mcps.json.migrate-bak"].map((file) => (
            <p
              key={file}
              className="flex items-center gap-1.5 font-mono text-[11.5px] text-zinc-400"
            >
              <FileWarning className="size-3.5 shrink-0 text-amber-300/80" />
              {file}
            </p>
          ))}
        </div>

        <p className="mt-3 text-[11.5px] leading-relaxed text-amber-200/90">
          {s.purgeBackupsRotateWarning}
        </p>

        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            disabled={purging}
            className="rounded-lg border border-white/10 bg-white/[0.03] px-3.5 py-2 text-[12px] font-medium text-zinc-300 transition hover:border-white/20 disabled:opacity-50"
          >
            {s.cancel}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={purging}
            className={cn(
              "rounded-lg bg-rose-600 px-3.5 py-2 text-[12px] font-semibold text-white transition",
              "hover:bg-rose-500 disabled:cursor-not-allowed disabled:opacity-50",
            )}
          >
            {purging ? s.purging : s.confirmPurgeBackups}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
