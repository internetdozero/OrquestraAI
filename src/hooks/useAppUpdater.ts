import { useCallback, useState } from "react";
import { useI18n } from "./useI18n";
import {
  applyAppUpdate,
  restartApp,
  syncOfficialExtensions,
} from "../lib/tauri";

interface AppUpdaterDeps {
  onToast: (message: string) => void;
  onError: (message: string) => void;
}

/** 1-click update state machine behind the About tab: app binary + extensions. */
export function useAppUpdater({ onToast, onError }: AppUpdaterDeps) {
  const { t } = useI18n();
  const a = t.settings.about;
  const [updatingApp, setUpdatingApp] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [updatedTo, setUpdatedTo] = useState<string | null>(null);

  const updateApp = useCallback(
    async (version: string) => {
      if (updatingApp) return;
      setUpdatingApp(true);
      try {
        await applyAppUpdate(version);
        setUpdatedTo(version);
        onToast(a.updateAppSuccess);
      } catch (cause) {
        const detail = cause instanceof Error ? cause.message : String(cause);
        onError(a.updateAppFailed(detail));
      } finally {
        setUpdatingApp(false);
      }
    },
    [a, onToast, onError, updatingApp],
  );

  const syncExtensions = useCallback(async () => {
    if (syncing) return;
    setSyncing(true);
    try {
      const result = await syncOfficialExtensions();
      onToast(
        a.syncExtensionsSuccess(
          result.updatedSkills.length + result.updatedMcps.length,
        ),
      );
    } catch (cause) {
      const detail = cause instanceof Error ? cause.message : String(cause);
      onError(a.updateAppFailed(detail));
    } finally {
      setSyncing(false);
    }
  }, [a, onToast, onError, syncing]);

  const restart = useCallback(() => {
    void restartApp().catch(() => {
      window.location.reload();
    });
  }, []);

  return { updatingApp, syncing, updatedTo, updateApp, syncExtensions, restart };
}
