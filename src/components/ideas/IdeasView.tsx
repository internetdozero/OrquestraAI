import { useState } from "react";
import { Lightbulb } from "lucide-react";
import { useIdeas } from "../../hooks/useIdeas";
import { useI18n } from "../../hooks/useI18n";
import { devMockMode } from "../../lib/fixtures";
import type { IdeaItem } from "../../lib/ideas";
import { ErrorBanner, StatePanel } from "../ui";
import { ConfirmModal } from "../ConfirmModal";
import { ArchivedIdeasModal } from "./ArchivedIdeasModal";
import { BlackboardFeed } from "./BlackboardFeed";
import { IdeaDetailModal } from "./IdeaDetailModal";
import { IdeaFormModal } from "./IdeaFormModal";
import { IdeasBoard } from "./IdeasBoard";
import { IdeasHeader, type BoardMode } from "./IdeasHeader";
import { ProductivitySubPanel } from "./ProductivitySubPanel";
import { IdeasListView } from "./IdeasListView";
import { IdeasToolbar } from "./IdeasToolbar";
import { PostNoteModal } from "./PostNoteModal";

interface IdeasViewProps {
  isTauri: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
}

/**
 * Full-width Ideas view: kanban or grouped list, detail viewer, and the
 * inter-agent blackboard behind the tab pills.
 */
export function IdeasView({ isTauri, onToast, onError }: IdeasViewProps) {
  const { t } = useI18n();
  const [mockMode] = useState(devMockMode);
  const state = useIdeas({ isTauri, onToast, onError });
  const s = t.ideas;
  const [mode, setMode] = useState<BoardMode>("board");
  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<IdeaItem | null>(null);
  const [detail, setDetail] = useState<IdeaItem | null>(null);
  const [noteOpen, setNoteOpen] = useState(false);
  const [archivedOpen, setArchivedOpen] = useState(false);

  /** Opening a task is what clears its [NEW] chip, in either view. */
  const openDetail = (idea: IdeaItem) => {
    state.markSeenIdea(idea.id);
    setDetail(idea);
  };
  const openCreate = () => {
    setEditing(null);
    setFormOpen(true);
  };
  const openEdit = (idea: IdeaItem) => {
    setEditing(idea);
    setFormOpen(true);
  };
  const advance = (idea: IdeaItem) => {
    const next =
      idea.status === "open"
        ? "in_progress"
        : idea.status === "in_progress"
          ? "completed"
          : idea.status === "completed"
            ? "archived"
            : null;
    if (next) void state.changeStatus(idea.id, next);
  };
  const [deleteTarget, setDeleteTarget] = useState<IdeaItem | null>(null);
  const confirmDelete = (idea: IdeaItem) => setDeleteTarget(idea);
  const archivedIdeas = state.ideas.filter((idea) => idea.status === "archived");

  if (!isTauri && !mockMode) {
    return (
      <div className="w-full max-w-none px-4">
        <StatePanel icon={Lightbulb} title={s.title} description={s.unavailable} />
      </div>
    );
  }

  return (
    <div className="w-full max-w-none space-y-3 px-4">
      {mockMode && (
        <p
          role="status"
          className="rounded-lg border border-cyan-500/25 bg-cyan-500/[0.06] px-3 py-1.5 font-mono text-[10.5px] text-cyan-200/90"
        >
          {s.mockBanner}
        </p>
      )}
      <IdeasHeader
        loading={state.loading}
        activeTab={state.activeTab}
        onTabChange={state.setActiveTab}
        mode={mode}
        onModeChange={setMode}
        onNewIdea={openCreate}
        onNewNote={() => setNoteOpen(true)}
        onRefresh={() => void state.refreshAll()}
      />

      <ProductivitySubPanel projects={state.projects} />

      {state.error && <ErrorBanner message={state.error} onDismiss={() => void state.refreshAll()} />}

      {state.activeTab === "ideas" ? (
        <>
          <IdeasToolbar
            search={state.search}
            onSearchChange={state.setSearch}
            projects={state.projects}
            selectedProject={state.selectedProject}
            onProjectChange={state.setSelectedProject}
            selectedCategory={state.selectedCategory}
            onCategoryChange={state.setSelectedCategory}
            selectedPriority={state.selectedPriority}
            onPriorityChange={state.setSelectedPriority}
            archivedCount={archivedIdeas.length}
            onOpenArchived={() => setArchivedOpen(true)}
          />

          {mode === "board" ? (
            <IdeasBoard
              ideas={state.ideas}
              loading={state.loading}
              newIds={state.newIds}
              onAdvance={advance}
              onEdit={openEdit}
              onDelete={confirmDelete}
              onOpen={openDetail}
              onDropIdea={(id, status) => void state.changeStatus(id, status)}
            />
          ) : (
            <IdeasListView
              ideas={state.ideas}
              loading={state.loading}
              newIds={state.newIds}
              onOpen={openDetail}
              onStatusChange={(id, status) => void state.changeStatus(id, status)}
              onEdit={openEdit}
              onDelete={confirmDelete}
            />
          )}
        </>
      ) : (
        <BlackboardFeed notes={state.notes} loading={state.loading} onMarkRead={(id) => void state.markRead(id)} />
      )}

      {archivedOpen && (
        <ArchivedIdeasModal
          ideas={archivedIdeas}
          onClose={() => setArchivedOpen(false)}
          onOpen={(idea) => {
            setArchivedOpen(false);
            openDetail(idea);
          }}
          onRestore={(id) => void state.changeStatus(id, "open")}
          onDelete={confirmDelete}
        />
      )}

      {detail && (
        <IdeaDetailModal
          idea={detail}
          onAdvance={advance}
          onEdit={openEdit}
          onDelete={confirmDelete}
          onClose={() => setDetail(null)}
        />
      )}

      {formOpen && (
        <IdeaFormModal
          idea={editing}
          projects={state.projects}
          saving={state.saving}
          onClose={() => setFormOpen(false)}
          onSave={async (input) => {
            if (editing) await state.update({ ...input, id: editing.id });
            else await state.create(input);
            setFormOpen(false);
          }}
        />
      )}

      {deleteTarget && (
        <ConfirmModal
          title={s.delete}
          message={s.deleteConfirm(deleteTarget.title)}
          confirmLabel={s.delete}
          cancelLabel={s.cancel}
          tone="danger"
          onCancel={() => setDeleteTarget(null)}
          onConfirm={() => {
            void state.remove(deleteTarget.id);
            setDeleteTarget(null);
          }}
        />
      )}

      {noteOpen && (
        <PostNoteModal
          projects={state.projects}
          saving={state.saving}
          onClose={() => setNoteOpen(false)}
          onSave={async (input) => {
            await state.postNote(input);
            setNoteOpen(false);
          }}
        />
      )}
    </div>
  );
}
