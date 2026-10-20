import { useCallback, useEffect, useMemo, useState } from "react";
import {
  createIdea as createIdeaCmd,
  deleteIdea as deleteIdeaCmd,
  listBlackboardNotes,
  listIdeaProjects,
  listIdeas,
  markNoteRead as markNoteReadCmd,
  postBlackboardNote,
  updateIdea as updateIdeaCmd,
  updateIdeaStatus,
  type BlackboardNote,
  type CreateIdeaInput,
  type IdeaItem,
  type PostNoteInput,
  type UpdateIdeaInput,
} from "../lib/ideas";
import { devMockMode } from "../lib/fixtures";
import { readIdeasCache, writeIdeasCache } from "../lib/ideasCache";
import { useI18n } from "./useI18n";

export type IdeasTab = "ideas" | "blackboard";

function toMessage(cause: unknown, fallback: string): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return fallback;
}

/** Next status in the `open → in_progress → completed → archived` chain. */
export function nextStatusOf(status: string): string | null {
  if (status === "open") return "in_progress";
  if (status === "in_progress") return "completed";
  if (status === "completed") return "archived";
  return null;
}

/**
 * All Ideas state in one place: vault rows, project list, blackboard notes
 * and the filters the toolbar drives. Filtering is client-side so typing
 * never round-trips through IPC; mutations refresh from the core.
 */
export function useIdeas(options: {
  isTauri: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
}) {
  const { isTauri, onToast, onError } = options;
  const { t } = useI18n();
  // Browser mock fixtures resolve through the same lib calls, so every
  // guard below treats mock mode like a live core.
  const [mockMode] = useState(devMockMode);
  const active = isTauri || mockMode;
  // SWR: paint instantly from the module cache when remounting, then
  // revalidate silently in the background instead of flashing empty.
  const initialCache = readIdeasCache();
  const [ideas, setIdeas] = useState<IdeaItem[]>(() => initialCache?.ideas ?? []);
  const [projects, setProjects] = useState<string[]>(() => initialCache?.projects ?? []);
  const [notes, setNotes] = useState<BlackboardNote[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<IdeasTab>("ideas");
  const [selectedProject, setSelectedProject] = useState("all");
  const [selectedCategory, setSelectedCategory] = useState("all");
  const [selectedPriority, setSelectedPriority] = useState("all");
  const [search, setSearch] = useState("");
  const [saving, setSaving] = useState(false);

  const refreshIdeas = useCallback(async () => {
    if (!active) return;
    setError(null);
    try {
      const [rows, names] = await Promise.all([
        listIdeas(null, "all"),
        listIdeaProjects(),
      ]);
      setIdeas(rows);
      setProjects(names);
      writeIdeasCache(rows, names);
    } catch (cause) {
      const message = t.ideas.loadFailed(toMessage(cause, t.common.unexpected));
      setError(message);
      onError(message);
    }
  }, [active, onError, t]);

  const refreshNotes = useCallback(async () => {
    if (!active) return;
    try {
      setNotes(await listBlackboardNotes(null, false));
    } catch (cause) {
      const message = t.ideas.loadFailed(toMessage(cause, t.common.unexpected));
      setError(message);
      onError(message);
    }
  }, [active, onError, t]);

  const refreshAll = useCallback(async () => {
    if (!active) return;
    setLoading(true);
    try {
      await Promise.all([refreshIdeas(), refreshNotes()]);
    } finally {
      setLoading(false);
    }
  }, [active, refreshIdeas, refreshNotes]);

  useEffect(() => {
    void refreshAll();
  }, [refreshAll]);

  // Auto-sync: background polling plus an immediate refresh when the window
  // regains focus or becomes visible. The timer is module-agnostic and torn
  // down on unmount so tab switches never stack intervals.
  useEffect(() => {
    if (!active) return;

    const interval = window.setInterval(() => {
      if (document.hidden) return;
      void refreshAll();
    }, 12_000);

    const onFocus = () => void refreshAll();
    const onVisibility = () => {
      if (document.visibilityState === "visible") void refreshAll();
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      window.clearInterval(interval);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [active, refreshAll]);

  const visible = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return ideas.filter((idea) => {
      if (selectedProject !== "all" && idea.project !== selectedProject) return false;
      if (selectedCategory !== "all" && idea.category !== selectedCategory) return false;
      if (selectedPriority !== "all" && idea.priority !== selectedPriority) return false;
      if (!needle) return true;
      return [idea.title, idea.description ?? "", idea.tags ?? ""].some((field) =>
        field.toLowerCase().includes(needle),
      );
    });
  }, [ideas, selectedProject, selectedCategory, selectedPriority, search]);

  const metrics = useMemo(() => {
    let open = 0;
    let inProgress = 0;
    let completed = 0;
    for (const idea of ideas) {
      if (idea.status === "open") open += 1;
      else if (idea.status === "in_progress") inProgress += 1;
      else if (idea.status === "completed") completed += 1;
    }
    return { total: ideas.length, open, inProgress, completed };
  }, [ideas]);

  const mutate = useCallback(
    async (work: () => Promise<unknown>, toast: string) => {
      if (!active || saving) return;
      setSaving(true);
      try {
        await work();
        await refreshIdeas();
        onToast(toast);
      } catch (cause) {
        onError(t.ideas.saveFailed(toMessage(cause, t.common.unexpected)));
        throw cause;
      } finally {
        setSaving(false);
      }
    },
    [active, saving, onToast, onError, refreshIdeas, t],
  );

  const create = useCallback(
    (input: CreateIdeaInput) => mutate(() => createIdeaCmd(input), t.ideas.savedToast),
    [mutate, t],
  );

  const update = useCallback(
    (input: UpdateIdeaInput) => mutate(() => updateIdeaCmd(input), t.ideas.updatedToast),
    [mutate, t],
  );

  const changeStatus = useCallback(
    (id: number, status: string) =>
      mutate(() => updateIdeaStatus(id, status), t.ideas.updatedToast),
    [mutate, t],
  );

  const remove = useCallback(
    (id: number) => mutate(() => deleteIdeaCmd(id), t.ideas.deletedToast),
    [mutate, t],
  );

  const postNote = useCallback(
    async (input: PostNoteInput) => {
      if (!active || saving) return;
      setSaving(true);
      try {
        await postBlackboardNote(input);
        await refreshNotes();
        onToast(t.ideas.postedToast);
      } catch (cause) {
        onError(t.ideas.saveFailed(toMessage(cause, t.common.unexpected)));
        throw cause;
      } finally {
        setSaving(false);
      }
    },
    [active, saving, onToast, onError, refreshNotes, t],
  );

  const markRead = useCallback(
    async (id: number) => {
      if (!active) return;
      try {
        await markNoteReadCmd(id);
        setNotes((rows) => rows.map((n) => (n.id === id ? { ...n, isRead: true } : n)));
        onToast(t.ideas.markedToast);
      } catch (cause) {
        onError(t.ideas.saveFailed(toMessage(cause, t.common.unexpected)));
      }
    },
    [active, onToast, onError, t],
  );

  return {
    ideas: visible,
    metrics,
    projects,
    notes,
    loading,
    saving,
    error,
    activeTab,
    setActiveTab,
    selectedProject,
    setSelectedProject,
    selectedCategory,
    setSelectedCategory,
    selectedPriority,
    setSelectedPriority,
    search,
    setSearch,
    refreshAll,
    create,
    update,
    changeStatus,
    remove,
    postNote,
    markRead,
  };
}

export type IdeasState = ReturnType<typeof useIdeas>;
