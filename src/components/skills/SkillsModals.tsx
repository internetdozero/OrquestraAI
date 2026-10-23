import type { SkillItem } from "../../lib/skills";
import { useI18n } from "../../hooks/useI18n";
import { ConfirmModal } from "../ConfirmModal";
import { EditSkillModal } from "./EditSkillModal";
import { ImportSkillModal } from "./ImportSkillModal";

interface SkillsModalsProps {
  isTauri: boolean;
  editingSkill: SkillItem | null;
  onCloseEditor: () => void;
  onSaved: (name: string) => void;
  onEditError: (message: string) => void;
  importOpen: boolean;
  onCloseImport: () => void;
  onImportError: (message: string) => void;
  onImported: (skill: { name: string }) => void;
  pendingDeleteSkill: SkillItem | null;
  onCancelDelete: () => void;
  onConfirmDelete: () => void;
}

/** Edit / import / delete-confirm modals for the skill grid. */
export function SkillsModals({
  isTauri,
  editingSkill,
  onCloseEditor,
  onSaved,
  onEditError,
  importOpen,
  onCloseImport,
  onImportError,
  onImported,
  pendingDeleteSkill,
  onCancelDelete,
  onConfirmDelete,
}: SkillsModalsProps) {
  const { t } = useI18n();

  return (
    <>
      {editingSkill && (
        <EditSkillModal
          skillId={editingSkill.id}
          skillName={editingSkill.name}
          onClose={onCloseEditor}
          onSaved={onSaved}
          onError={onEditError}
        />
      )}

      {importOpen && (
        <ImportSkillModal
          isTauri={isTauri}
          onClose={onCloseImport}
          onError={onImportError}
          onImported={onImported}
        />
      )}

      {pendingDeleteSkill && (
        <ConfirmModal
          title={t.skills.deleteTitle}
          message={t.skills.deleteConfirm(pendingDeleteSkill.name)}
          confirmLabel={t.common.delete}
          cancelLabel={t.skills.deleteCancel}
          tone="danger"
          onCancel={onCancelDelete}
          onConfirm={onConfirmDelete}
        />
      )}
    </>
  );
}
