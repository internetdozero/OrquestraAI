import { useCallback, useEffect, useMemo, useState } from "react";
import {
  IDEAS_CATALOG_KEY,
  IDEAS_CATALOG_TTL_MS,
  IDEAS_NOTES_KEY,
  IDEAS_NOTES_TTL_MS,
  createIdea as createIdeaCmd,
  deleteIdea as deleteIdeaCmd,
  loadBlackboardNotes,
  loadIdeasCatalog,
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
import { collectNewIdeaIds, readSeenIdeaIds, writeSeenIdeaIds } from "../lib/seenIdeas";
import { useCachedResource } from "./useCachedResource";
import { useI18n } from "./useI18n";

export type IdeasTab = "ideas" | "blackboard";

/** Stable identities: a fresh `{ ideas: [], projects: [] }` per render would
 *  reset the resource state in `useCachedResource`. */
const EMPTY_CATALOG: { ideas: IdeaItem[]; projects: string[] } = { ideas: [], projects: [] };
const EMPTY_NOTES: BlackboardNote[] = [];

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
 * and the filters the toolbar drives.
 *
 * Reads go through the shared TTL cache (`useCachedResource`): a remount paints
 * the previous rows in the first render, a value younger than 30 s is served
 * without touching the core at all, and an older one is repainted in place
 * while a background read runs — no spinner unless this is genuinely the first
 * read of the session. Filtering stays client-side, so typing never
 * round-trips through IPC.
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

  // The loaders and their keys live in `lib/ideas` so the boot warm-up can
  // fill exactly the entries this view reads.
  const catalog = useCachedResource<{ ideas: IdeaItem[]; projects: string[] }>({
    key: IDEAS_CATALOG_KEY,
    ttlMs: IDEAS_CATALOG_TTL_MS,
    enabled: active,
    fallback: EMPTY_CATALOG,
    load: loadIdeasCatalog,
  });

  const notesResource = useCachedResource<BlackboardNote[]>({
    key: IDEAS_NOTES_KEY,
    ttlMs: IDEAS_NOTES_TTL_MS,
    enabled: active,
    fallback: EMPTY_NOTES,
    load: loadBlackboardNotes,
  });

  const ideas = catalog.data.ideas;
  const projects = catalog.data.projects;
  const notes = notesResource.data;
  const loading = catalog.loading || notesResource.loading;

  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<IdeasTab>("ideas");
  const [selectedProject, setSelectedProject] = useState("all");
  const [selectedCategory, setSelectedCategory] = useState("all");
  const [selectedPriority, setSelectedPriority] = useState("all");
  const [search, setSearch] = useState("");
  const [saving, setSaving] = useState(false);
  // Ids the operator already interacted with; the [NEW] chip keys off this set.
  const [seenIds, setSeenIds] = useState<ReadonlySet<number>>(() => new Set(readSeenIdeaIds()));

  // Destructured: the resource objects are re-created every render, and an
  // unstable callback here would re-arm the polling effect on each pass.
  const { refresh: reloadCatalog, ensure: ensureCatalog } = catalog;
  const { refresh: reloadNotes, ensure: ensureNotes, mutate: mutateNotes } = notesResource;
  const failure = catalog.error ?? notesResource.error;

  // One place turns either resource's failure into the operator-facing banner.
  useEffect(() => {
    if (!failure) return;
    const message = t.ideas.loadFailed(toMessage(failure, t.common.unexpected));
    setError(message);
    onError(message);
  }, [failure, onError, t]);

  /** TTL-aware revalidation: polling and focus only read when the data aged. */
  const revalidate = useCallback(async () => {
    if (!active) return;
    await Promise.all([ensureCatalog(), ensureNotes()]);
  }, [active, ensureCatalog, ensureNotes]);

  /** Explicit refresh (header button, mutation): always reads from the core. */
  const refreshAll = useCallback(async () => {
    if (!active) return;
    setError(null);
    await Promise.all([reloadCatalog(), reloadNotes()]);
  }, [active, reloadCatalog, reloadNotes]);

  // Auto-sync: a slow 60 s safety net plus an immediate revalidation when the
  // window regains focus or becomes visible (the event-driven part), plus the
  // header's manual refresh button. Mutations already revalidate eagerly, so
  // the interval only catches edits from outside this window (the idea-vault
  // MCP, another agent) — and it reuses the cache TTL, so a focus storm (window
  // managers can fire several) never becomes an IPC storm. The timer is torn
  // down on unmount so tab switches never stack intervals.
  useEffect(() => {
    if (!active) return;

    const interval = window.setInterval(() => {
      if (document.hidden) return;
      void revalidate();
    }, 60_000);

    const onFocus = () => void revalidate();
    const onVisibility = () => {
      if (document.visibilityState === "visible") void revalidate();
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      window.clearInterval(interval);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [active, revalidate]);

  const visible = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return ideas.filter((idea) => {
      if (selectedProject !== "all" && idea.project !== selectedProject) return false;
      if (selectedCategory !== "all" && idea.category !== selectedCategory) return false;
      if (selectedPriority !== "all" && idea.priority !== selectedPriority) return false;
      if (!needle) return true;
      return [idea.title, idea.description ?? "", idea.tags ?? "", idea.prompt ?? ""].some(
        (field) => field.toLowerCase().includes(needle),
      );
    });
  }, [ideas, selectedProject, selectedCategory, selectedPriority, search]);

  /**
   * Any operator interaction clears [NEW] — opening, copying the AI prompt,
   * editing, advancing, deleting or moving the card. One persisted set for
   * every entry point, so the chip disappears for good instead of returning
   * on the next poll.
   */
  const noteInteraction = useCallback((id: number) => {
    setSeenIds((current) => {
      if (current.has(id)) return current;
      const next = new Set(current).add(id);
      writeSeenIdeaIds(next);
      return next;
    });
  }, []);

  /** Opening a task is one interaction among others; kept as a named entry point. */
  const markSeenIdea = useCallback((id: number) => noteInteraction(id), [noteInteraction]);

  // Recomputed on every poll, so a chip expires within one refresh cycle of
  // the freshness window closing.
  const newIds = useMemo(() => collectNewIdeaIds(ideas, seenIds), [ideas, seenIds]);

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
        await reloadCatalog();
        onToast(toast);
      } catch (cause) {
        onError(t.ideas.saveFailed(toMessage(cause, t.common.unexpected)));
        throw cause;
      } finally {
        setSaving(false);
      }
    },
    [active, saving, onToast, onError, reloadCatalog, t],
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
        await reloadNotes();
        onToast(t.ideas.postedToast);
      } catch (cause) {
        onError(t.ideas.saveFailed(toMessage(cause, t.common.unexpected)));
        throw cause;
      } finally {
        setSaving(false);
      }
    },
    [active, saving, onToast, onError, reloadNotes, t],
  );

  const markRead = useCallback(
    async (id: number) => {
      if (!active) return;
      try {
        await markNoteReadCmd(id);
        // Optimistic: the optimistic write bumps the cache generation, so a
        // read that started before it cannot flip the row back to unread.
        mutateNotes(notes.map((note) => (note.id === id ? { ...note, isRead: true } : note)));
        onToast(t.ideas.markedToast);
      } catch (cause) {
        onError(t.ideas.saveFailed(toMessage(cause, t.common.unexpected)));
      }
    },
    [active, onToast, onError, notes, mutateNotes, t],
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
    newIds,
    markSeenIdea,
    noteInteraction,
    create,
    update,
    changeStatus,
    remove,
    postNote,
    markRead,
  };
}

export type IdeasState = ReturnType<typeof useIdeas>;
