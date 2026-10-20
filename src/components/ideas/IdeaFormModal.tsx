import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Loader2, Save, X } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import type { CreateIdeaInput, IdeaItem, UpdateIdeaInput } from "../../lib/ideas";
import { CustomDropdown } from "../CustomDropdown";
import { cn, confirmOnEnter } from "../../lib/utils";

const inputClass =
  "h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-2.5 text-[12px] text-zinc-200 placeholder:text-zinc-600 outline-none transition focus:border-amber-500/40 focus:bg-zinc-900 focus:ring-2 focus:ring-amber-500/10";
const labelClass =
  "mb-1 block text-[10px] font-medium tracking-[0.12em] text-zinc-500 uppercase";

const CATEGORY_IDS = ["feature", "architecture", "bug", "ux", "idea"] as const;
const PRIORITY_IDS = ["low", "medium", "high", "critical"] as const;
const STATUS_IDS = ["open", "in_progress", "completed", "archived"] as const;

interface IdeaFormModalProps {
  /** `null` opens the modal in create mode. */
  idea: IdeaItem | null;
  projects: string[];
  saving: boolean;
  onClose: () => void;
  onSave: (input: CreateIdeaInput & Partial<UpdateIdeaInput>) => Promise<void>;
}

/** Create/edit modal, portaled to `document.body` so no ancestor can clip it. */
export function IdeaFormModal({ idea, projects, saving, onClose, onSave }: IdeaFormModalProps) {
  const { t } = useI18n();
  const s = t.ideas;
  const creating = idea === null;

  const [project, setProject] = useState(idea?.project ?? projects[0] ?? "");
  const [title, setTitle] = useState(idea?.title ?? "");
  const [description, setDescription] = useState(idea?.description ?? "");
  const [category, setCategory] = useState(idea?.category ?? "feature");
  const [priority, setPriority] = useState(idea?.priority ?? "medium");
  const [status, setStatus] = useState(idea?.status ?? "open");
  const [tags, setTags] = useState(idea?.tags ?? "");
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
    if (!title.trim()) return setLocalError(s.titleRequired);
    if (!project.trim()) return setLocalError(s.fieldProject);
    setLocalError(null);
    void onSave({
      project: project.trim(),
      title: title.trim(),
      description: description.trim() || null,
      category,
      priority,
      status,
      tags: tags.trim() || null,
    }).catch(() => undefined);
  };

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={creating ? s.formCreateTitle : s.formEditTitle}
      className="fixed inset-0 z-[100] flex items-start justify-center overflow-y-auto bg-black/70 px-4 py-10 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !saving) onClose();
      }}
    >
      <div
        className="w-full max-w-xl overflow-hidden rounded-xl border border-white/[0.08] bg-[#0d0d10] shadow-[0_30px_80px_-30px_rgba(0,0,0,0.9)]"
        onKeyDown={(event) => confirmOnEnter(event, submit)}
      >
        <div className="flex items-start justify-between gap-4 border-b border-white/[0.06] px-5 py-4">
          <div>
            <h2 className="text-sm font-semibold tracking-tight text-zinc-100">
              {creating ? s.formCreateTitle : s.formEditTitle}
            </h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">{s.formBlurb}</p>
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
              <label className={labelClass} htmlFor="idea-project">
                {s.fieldProject}
              </label>
              <input
                id="idea-project"
                value={project}
                onChange={(event) => setProject(event.target.value)}
                list="idea-projects"
                spellCheck={false}
                className={cn(inputClass, "font-mono")}
              />
              <datalist id="idea-projects">
                {projects.map((name) => (
                  <option key={name} value={name} />
                ))}
              </datalist>
            </div>
            <div>
              <span className={labelClass} id="idea-status-label">
                {s.fieldStatus}
              </span>
              <CustomDropdown
                value={status}
                ariaLabel={s.fieldStatus}
                options={STATUS_IDS.map((id) => ({
                  id,
                  label: s.statusNames[id] ?? id,
                }))}
                onChange={setStatus}
                className="w-full [&>button]:w-full [&>button]:justify-between"
              />
            </div>
          </div>

          <div>
            <label className={labelClass} htmlFor="idea-title">
              {s.fieldTitle}
            </label>
            <input
              id="idea-title"
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              spellCheck={false}
              className={inputClass}
            />
          </div>

          <div>
            <label className={labelClass} htmlFor="idea-description">
              {s.fieldDescription}
            </label>
            <textarea
              id="idea-description"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              rows={4}
              className={cn(inputClass, "h-auto resize-y py-2 leading-relaxed")}
            />
          </div>

          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div>
              <span className={labelClass}>{s.fieldCategory}</span>
              <CustomDropdown
                value={category}
                ariaLabel={s.fieldCategory}
                options={CATEGORY_IDS.map((id) => ({ id, label: s.categories[id] }))}
                onChange={setCategory}
                className="w-full [&>button]:w-full [&>button]:justify-between"
              />
            </div>
            <div>
              <span className={labelClass}>{s.fieldPriority}</span>
              <CustomDropdown
                value={priority}
                ariaLabel={s.fieldPriority}
                options={PRIORITY_IDS.map((id) => ({ id, label: s.priorities[id] }))}
                onChange={setPriority}
                className="w-full [&>button]:w-full [&>button]:justify-between"
              />
            </div>
          </div>

          <div>
            <label className={labelClass} htmlFor="idea-tags">
              {s.fieldTags} <span className="text-zinc-600">· {s.tagsHint}</span>
            </label>
            <input
              id="idea-tags"
              value={tags}
              onChange={(event) => setTags(event.target.value)}
              spellCheck={false}
              className={cn(inputClass, "font-mono")}
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
            {saving ? <Loader2 className="size-3.5 animate-spin" /> : <Save className="size-3.5" />}
            {s.save}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
