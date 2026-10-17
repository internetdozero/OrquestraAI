import { useCallback, useEffect, useState } from "react";
import { localizePresetWarning, presentPreset } from "../i18n/presetCopy";
import { useI18n } from "./useI18n";
import {
  applyPreset,
  deletePreset,
  listPresets,
  savePreset,
  type Preset,
  type PresetReport,
} from "../lib/presets";

function toMessage(cause: unknown, fallback: string): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return fallback;
}

interface UsePresetsOptions {
  isTauri: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
  /** Runs after a successful apply so the shell can re-read what changed. */
  onApplied: (report: PresetReport) => void;
}

/**
 * Built-in and custom mission presets, plus the one-click apply. Apply moves the
 * active highlight immediately and only rolls back to the core's truth if the
 * command fails, so the bar feels instant while the Rust side rewrites
 * projections and harness configs.
 */
export function usePresets({
  isTauri,
  onToast,
  onError,
  onApplied,
}: UsePresetsOptions) {
  const { t } = useI18n();
  const [presets, setPresets] = useState<Preset[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    if (!isTauri) {
      setLoading(false);
      return;
    }

    try {
      setPresets(await listPresets());
    } catch (cause) {
      onError(t.presets.readFailed(toMessage(cause, t.common.unexpected)));
    } finally {
      setLoading(false);
    }
  }, [isTauri, onError, t]);

  useEffect(() => {
    void load();
  }, [load]);

  const apply = useCallback(
    async (preset: Preset) => {
      if (busyId) return;

      setBusyId(preset.id);
      setPresets((current) =>
        current.map((item) => ({ ...item, active: item.id === preset.id })),
      );

      try {
        const report = await applyPreset(preset.id);
        const shown = presentPreset(t, preset);
        onToast(
          t.presets.applied(shown.name, report.skillsEnabled, report.mcpsEnabled),
        );
        if (report.warnings.length > 0) {
          onError(report.warnings.map((warning) => localizePresetWarning(t, warning)).join("; "));
        }
        onApplied(report);
      } catch (cause) {
        onError(
          t.presets.applyFailed(
            presentPreset(t, preset).name,
            toMessage(cause, t.common.unexpected),
          ),
        );
        await load();
      } finally {
        setBusyId(null);
      }
    },
    [busyId, onToast, onError, onApplied, load, t],
  );

  /** Resolves `true` when the profile was saved, so the modal can close. */
  const save = useCallback(
    async (name: string, description: string): Promise<boolean> => {
      setSaving(true);
      try {
        const preset = await savePreset(name, description.trim() || null);
        setPresets((current) => [
          ...current.map((item) => ({ ...item, active: false })),
          { ...preset, active: true },
        ]);
        onToast(t.presets.saved(presentPreset(t, preset).name));
        return true;
      } catch (cause) {
        onError(t.presets.saveFailed(toMessage(cause, t.common.unexpected)));
        return false;
      } finally {
        setSaving(false);
      }
    },
    [onToast, onError, t],
  );

  const remove = useCallback(
    async (preset: Preset) => {
      try {
        await deletePreset(preset.id);
        setPresets((current) =>
          current
            .filter((item) => item.id !== preset.id)
            .map((item) => (preset.active ? { ...item, active: false } : item)),
        );
        onToast(t.presets.deleted(presentPreset(t, preset).name));
      } catch (cause) {
        onError(
          t.presets.deleteFailed(
            presentPreset(t, preset).name,
            toMessage(cause, t.common.unexpected),
          ),
        );
      }
    },
    [onToast, onError, t],
  );

  return { presets, loading, busyId, saving, apply, save, remove, reload: load };
}
