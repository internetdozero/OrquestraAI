import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Loader2, X } from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import { readSkillContent, saveSkillContent } from "../../lib/tauri";
import { confirmOnEnter } from "../../lib/utils";

interface EditSkillModalProps {
  skillId: string;
  skillName: string;
  onClose: () => void;
  onSaved: (name: string) => void;
  onError: (message: string) => void;
}

function toMessage(cause: unknown): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return String(cause);
}

function splitDoc(content: string): { fm: string[] | null; body: string } {
  const lines = content.split("\n");
  if (lines[0]?.trim() !== "---") return { fm: null, body: content };
  const end = lines.findIndex(
    (line, i) => i > 0 && (line.trim() === "---" || line.trim() === "..."),
  );
  if (end < 0) return { fm: null, body: content };
  return { fm: lines.slice(1, end), body: lines.slice(end + 1).join("\n") };
}

/** First top-level `key:` value plus its indented continuations, collapsed. */
function fmValue(fm: string[], key: string): string {
  const idx = fm.findIndex((line) => new RegExp(`^${key}\\s*:`).test(line));
  if (idx < 0) return "";
  const parts = [fm[idx].replace(new RegExp(`^${key}\\s*:\\s*`), "")];
  for (let i = idx + 1; i < fm.length; i += 1) {
    if (!/^\s/.test(fm[i]) || fm[i].trim() === "") break;
    parts.push(fm[i].trim());
  }
  return parts
    .join(" ")
    .replace(/^[>|][-+]?\s*/, "")
    .trim();
}

function yamlScalar(value: string): string {
  return /[:#"\n]/.test(value) || /^\s|\s$/.test(value) || value === ""
    ? JSON.stringify(value)
    : value;
}

/** Replace a top-level key in place (dropping stale continuations), or insert. */
function setFmValue(fm: string[], key: string, value: string): string[] {
  const next = [...fm];
  let at = 0;
  const idx = next.findIndex((line) => new RegExp(`^${key}\\s*:`).test(line));
  if (idx >= 0) {
    at = idx;
    next.splice(idx, 1);
    while (at < next.length && /^\s/.test(next[at]) && next[at].trim() !== "") {
      next.splice(at, 1);
    }
  }
  next.splice(at, 0, `${key}: ${yamlScalar(value)}`);
  return next;
}

/** Editor for one vault skill: name, description and the SKILL.md body. */
export function EditSkillModal({
  skillId,
  skillName,
  onClose,
  onSaved,
  onError,
}: EditSkillModalProps) {
  const { t } = useI18n();
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [hadFrontmatter, setHadFrontmatter] = useState(true);
  const [fmLines, setFmLines] = useState<string[]>([]);
  const [name, setName] = useState(skillName);
  const [description, setDescription] = useState("");
  const [body, setBody] = useState("");

  useEffect(() => {
    let active = true;
    void readSkillContent(skillId)
      .then((content) => {
        if (!active) return;
        const { fm, body: text } = splitDoc(content);
        setHadFrontmatter(fm !== null);
        const lines = fm ?? [];
        setFmLines(lines);
        setName(fmValue(lines, "name") || skillName);
        setDescription(fmValue(lines, "description"));
        setBody(text.replace(/^\n/, ""));
      })
      .catch((cause: unknown) => {
        if (active) {
          onError(t.skills.editFailed(toMessage(cause)));
          onClose();
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [skillId, skillName, onClose, onError, t]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, saving]);

  const save = async () => {
    const trimmedName = name.trim();
    if (saving || !trimmedName) return;
    setSaving(true);
    try {
      const fm = setFmValue(
        setFmValue(hadFrontmatter ? fmLines : [], "name", trimmedName),
        "description",
        description.trim(),
      );
      const content = `---\n${fm.join("\n")}\n---\n${body}`;
      await saveSkillContent(skillId, content);
      onSaved(trimmedName);
      onClose();
    } catch (cause) {
      onError(t.skills.editFailed(toMessage(cause)));
    } finally {
      setSaving(false);
    }
  };

  if (typeof document === "undefined") return null;

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t.skills.editHeading}
      className="fixed inset-0 z-[100] flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !saving) onClose();
      }}
    >
      <div
        className="flex max-h-[86vh] w-full max-w-xl flex-col overflow-hidden rounded-2xl border border-white/10 bg-[#0e0e12] shadow-[0_30px_80px_-20px_rgba(0,0,0,0.9)]"
        onKeyDown={(event) => confirmOnEnter(event, () => void save())}
      >
        <div className="flex items-start justify-between gap-4 border-b border-white/[0.06] px-5 py-4">
          <div>
            <h2 className="text-sm font-semibold text-zinc-100">
              {t.skills.editHeading} · <span className="font-mono">{skillId}</span>
            </h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            disabled={saving}
            aria-label={t.common.close}
            className="rounded-md p-1.5 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200 disabled:opacity-40"
          >
            <X className="size-4" />
          </button>
        </div>

        {loading ? (
          <p className="flex items-center gap-2 px-5 py-10 text-[12px] text-zinc-500">
            <Loader2 className="size-4 animate-spin" />
            {t.skills.editLoading}
          </p>
        ) : (
          <div className="scroll-pane min-h-0 flex-1 space-y-3 overflow-y-auto px-5 py-4">
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <label className="block">
                <span className="mb-1 block text-[10px] font-medium tracking-[0.12em] text-zinc-500 uppercase">
                  {t.skills.editNameLabel}
                </span>
                <input
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  spellCheck={false}
                  className="h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-2.5 text-[12.5px] text-zinc-200 outline-none focus:border-emerald-500/40"
                />
              </label>
              <label className="block">
                <span className="mb-1 block text-[10px] font-medium tracking-[0.12em] text-zinc-500 uppercase">
                  {t.skills.editDescLabel}
                </span>
                <input
                  value={description}
                  onChange={(event) => setDescription(event.target.value)}
                  spellCheck={false}
                  className="h-9 w-full rounded-lg border border-white/10 bg-zinc-900/60 px-2.5 text-[12.5px] text-zinc-200 outline-none focus:border-emerald-500/40"
                />
              </label>
            </div>
            <label className="block">
              <span className="mb-1 block text-[10px] font-medium tracking-[0.12em] text-zinc-500 uppercase">
                {t.skills.editBodyLabel}
              </span>
              <textarea
                value={body}
                onChange={(event) => setBody(event.target.value)}
                rows={14}
                spellCheck={false}
                className="w-full resize-y rounded-lg border border-white/10 bg-zinc-900/60 px-3 py-2 font-mono text-[12px] leading-relaxed text-zinc-200 outline-none focus:border-emerald-500/40"
              />
            </label>
          </div>
        )}

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
            onClick={() => void save()}
            disabled={saving || loading || !name.trim()}
            className="flex items-center gap-1.5 rounded-lg border border-emerald-500/30 bg-emerald-500/15 px-3 py-1.5 text-[12px] font-medium text-emerald-200 transition hover:bg-emerald-500/25 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {saving && <Loader2 className="size-3.5 animate-spin" />}
            {t.skills.editSave}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
