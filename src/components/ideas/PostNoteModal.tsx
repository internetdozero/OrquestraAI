import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Loader2, Send, X } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import type { PostNoteInput } from "../../lib/ideas";

const inputClass =
  "h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-2.5 text-[12px] text-zinc-200 placeholder:text-zinc-600 outline-none transition focus:border-amber-500/40 focus:bg-zinc-900 focus:ring-2 focus:ring-amber-500/10";
const labelClass =
  "mb-1 block text-[10px] font-medium tracking-[0.12em] text-zinc-500 uppercase";

interface PostNoteModalProps {
  projects: string[];
  saving: boolean;
  onClose: () => void;
  onSave: (input: PostNoteInput) => Promise<void>;
}

/** Post-note modal, portaled to `document.body` so no ancestor can clip it. */
export function PostNoteModal({ projects, saving, onClose, onSave }: PostNoteModalProps) {
  const { t } = useI18n();
  const s = t.ideas;

  const [project, setProject] = useState(projects[0] ?? "");
  const [topic, setTopic] = useState("");
  const [sender, setSender] = useState("");
  const [target, setTarget] = useState("all");
  const [note, setNote] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, saving]);

  const submit = () => {
    if (saving) return;
    if (!project.trim() || !topic.trim() || !sender.trim() || !note.trim()) {
      return setLocalError(s.noteRequired);
    }
    setLocalError(null);
    void onSave({
      project: project.trim(),
      topic: topic.trim(),
      sender: sender.trim(),
      target: target.trim() || null,
      note: note.trim(),
    }).catch(() => undefined);
  };

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={s.postTitle}
      className="fixed inset-0 z-[100] flex items-start justify-center overflow-y-auto bg-black/70 px-4 py-10 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !saving) onClose();
      }}
    >
      <div className="w-full max-w-xl overflow-hidden rounded-xl border border-white/[0.08] bg-[#0d0d10] shadow-[0_30px_80px_-30px_rgba(0,0,0,0.9)]">
        <div className="flex items-start justify-between gap-4 border-b border-white/[0.06] px-5 py-4">
          <div>
            <h2 className="text-sm font-semibold tracking-tight text-zinc-100">{s.postTitle}</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">{s.postBlurb}</p>
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
          <div>
            <label className={labelClass} htmlFor="note-project">
              {s.fieldProject}
            </label>
            <input
              id="note-project"
              value={project}
              onChange={(event) => setProject(event.target.value)}
              list="note-projects"
              spellCheck={false}
              className={`${inputClass} font-mono`}
            />
            <datalist id="note-projects">
              {projects.map((name) => (
                <option key={name} value={name} />
              ))}
            </datalist>
          </div>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
            <div>
              <label className={labelClass} htmlFor="note-topic">
                {s.fieldTopic}
              </label>
              <input
                id="note-topic"
                value={topic}
                onChange={(event) => setTopic(event.target.value)}
                spellCheck={false}
                className={`${inputClass} font-mono`}
              />
            </div>
            <div>
              <label className={labelClass} htmlFor="note-sender">
                {s.fieldSender}
              </label>
              <input
                id="note-sender"
                value={sender}
                onChange={(event) => setSender(event.target.value)}
                spellCheck={false}
                className={`${inputClass} font-mono`}
              />
            </div>
            <div>
              <label className={labelClass} htmlFor="note-target">
                {s.fieldTarget}
              </label>
              <input
                id="note-target"
                value={target}
                onChange={(event) => setTarget(event.target.value)}
                spellCheck={false}
                className={`${inputClass} font-mono`}
              />
            </div>
          </div>

          <div>
            <label className={labelClass} htmlFor="note-body">
              {s.fieldNote}
            </label>
            <textarea
              id="note-body"
              value={note}
              onChange={(event) => setNote(event.target.value)}
              rows={5}
              className={`${inputClass} h-auto resize-y py-2 leading-relaxed`}
            />
          </div>

          {localError && (
            <p className="rounded-lg border border-amber-500/25 bg-amber-500/[0.07] px-3 py-2 text-[11.5px] text-amber-200">
              {localError}
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
            {s.cancel}
          </button>
          <button
            type="button"
            onClick={submit}
            disabled={saving}
            className="flex items-center gap-1.5 rounded-lg border border-amber-500/30 bg-amber-500/15 px-3 py-1.5 text-[12px] font-medium text-amber-200 transition hover:bg-amber-500/25 disabled:cursor-wait disabled:opacity-60"
          >
            {saving ? <Loader2 className="size-3.5 animate-spin" /> : <Send className="size-3.5" />}
            {s.save}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
