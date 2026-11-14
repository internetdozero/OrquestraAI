import { useCallback, useEffect, useRef, useState } from "react";
import { useI18n } from "./useI18n";
import { useAutoUpdateCheck } from "./useAutoUpdateCheck";
import {
  downloadAppUpdate,
  installStagedUpdate,
  listenAppUpdateProgress,
  restartApp,
  syncOfficialExtensions,
  type AppUpdateProgress,
} from "../lib/tauri";
import { checkLatestRelease, type ReleaseCheckResult } from "../lib/updater";

interface AppUpdaterDeps {
  enabled: boolean;
  currentVersion: string | null;
  isTauri: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
}

export type AppUpdateStep =
  | "idle"
  | "downloading"
  | "verifying"
  | "ready"
  | "installing"
  | "done"
  | "error";

export interface AppUpdaterState {
  checking: boolean;
  checkResult: ReleaseCheckResult | null;
  availableRelease: ReleaseCheckResult | null;
  step: AppUpdateStep;
  updatingApp: boolean;
  progress: AppUpdateProgress | null;
  readyVersion: string | null;
  updatedTo: string | null;
  promptOpen: boolean;
  syncing: boolean;
  checkNow: () => Promise<ReleaseCheckResult | null>;
  updateApp: (version: string) => Promise<void>;
  installReadyAndRestart: () => Promise<void>;
  dismissPrompt: () => void;
  syncExtensions: () => Promise<void>;
  restart: () => void;
}

/** App-wide idempotent auto-updater: 30-min check, background download & prompt. */
export function useAppUpdater({
  enabled,
  currentVersion,
  isTauri,
  onToast,
  onError,
}: AppUpdaterDeps): AppUpdaterState {
  const { t } = useI18n();
  const a = t.settings.about;

  const [checking, setChecking] = useState(false);
  const [checkResult, setCheckResult] = useState<ReleaseCheckResult | null>(null);
  const [availableRelease, setAvailableRelease] = useState<ReleaseCheckResult | null>(null);
  const [step, setStep] = useState<AppUpdateStep>("idle");
  const [progress, setProgress] = useState<AppUpdateProgress | null>(null);
  const [readyVersion, setReadyVersion] = useState<string | null>(null);
  const [updatedTo, setUpdatedTo] = useState<string | null>(null);
  const [promptOpen, setPromptOpen] = useState(false);
  const [syncing, setSyncing] = useState(false);

  const dismissedVersionRef = useRef<string | null>(null);
  const readyVersionRef = useRef<string | null>(null);
  readyVersionRef.current = readyVersion;
  const updatedToRef = useRef<string | null>(null);
  updatedToRef.current = updatedTo;
  const inFlightCheckRef = useRef<Promise<ReleaseCheckResult | null> | null>(null);
  const inFlightDownloadRef = useRef<Promise<string | null> | null>(null);
  const inFlightInstallRef = useRef<Promise<void> | null>(null);

  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    void listenAppUpdateProgress((payload) => {
      setProgress(payload);
      if (payload.phase === "downloading") setStep("downloading");
      else if (payload.phase === "verifying") setStep("verifying");
      else if (payload.phase === "ready") setStep("ready");
      else if (payload.phase === "installing") setStep("installing");
      else if (payload.phase === "installed") setStep("done");
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [isTauri]);

  const restart = useCallback(() => {
    void restartApp().catch(() => {
      window.location.reload();
    });
  }, []);

  const stageVersion = useCallback(
    (version: string, interactive: boolean): Promise<string | null> => {
      if (!isTauri) return Promise.resolve(null);
      if (updatedToRef.current === version) return Promise.resolve(version);
      if (readyVersionRef.current === version) {
        if (dismissedVersionRef.current !== version) setPromptOpen(true);
        return Promise.resolve(version);
      }
      if (inFlightDownloadRef.current) return inFlightDownloadRef.current;

      setStep("downloading");
      setProgress({ version, phase: "downloading", downloadedBytes: 0, totalBytes: null });
      const task = downloadAppUpdate(version)
        .then((staged) => {
          setReadyVersion(staged);
          setStep("ready");
          if (dismissedVersionRef.current !== staged) {
            setPromptOpen(true);
          }
          return staged;
        })
        .catch((cause) => {
          setStep("error");
          if (interactive) {
            const detail = cause instanceof Error ? cause.message : String(cause);
            onError(a.updateAppFailed(detail));
          }
          return null;
        })
        .finally(() => {
          inFlightDownloadRef.current = null;
        });
      inFlightDownloadRef.current = task;
      return task;
    },
    [a, isTauri, onError],
  );

  const handleBackgroundUpdateFound = useCallback(
    (release: ReleaseCheckResult) => {
      setAvailableRelease(release);
      setCheckResult(release);
      void stageVersion(release.latestVersion, false);
    },
    [stageVersion],
  );

  useAutoUpdateCheck({
    enabled,
    currentVersion,
    isTauri,
    onUpdateFound: handleBackgroundUpdateFound,
  });

  const checkNow = useCallback((): Promise<ReleaseCheckResult | null> => {
    if (!currentVersion) return Promise.resolve(null);
    if (inFlightCheckRef.current) return inFlightCheckRef.current;

    setChecking(true);
    const task = checkLatestRelease(currentVersion)
      .then((res) => {
        setCheckResult(res);
        if (!res.isLatest) {
          setAvailableRelease(res);
        }
        return res;
      })
      .finally(() => {
        setChecking(false);
        inFlightCheckRef.current = null;
      });
    inFlightCheckRef.current = task;
    return task;
  }, [currentVersion]);

  const installVersion = useCallback(
    (version: string, shouldRestart: boolean): Promise<void> => {
      if (!isTauri) return Promise.resolve();
      if (updatedToRef.current === version) {
        if (shouldRestart) restart();
        return Promise.resolve();
      }
      if (inFlightInstallRef.current) return inFlightInstallRef.current;

      const task = (async () => {
        try {
          if (readyVersionRef.current !== version) {
            const staged = await stageVersion(version, true);
            if (!staged) return;
          }
          setStep("installing");
          await installStagedUpdate(version);
          setUpdatedTo(version);
          setReadyVersion(null);
          setPromptOpen(false);
          setStep("done");
          onToast(a.updateAppSuccess);
          if (shouldRestart) {
            restart();
          }
        } catch (cause) {
          const detail = cause instanceof Error ? cause.message : String(cause);
          setStep("error");
          onError(a.updateAppFailed(detail));
        } finally {
          inFlightInstallRef.current = null;
        }
      })();
      inFlightInstallRef.current = task;
      return task;
    },
    [a, isTauri, onError, onToast, restart, stageVersion],
  );

  const updateApp = useCallback(
    (version: string) => installVersion(version, false),
    [installVersion],
  );

  const installReadyAndRestart = useCallback(() => {
    const target = readyVersionRef.current ?? availableRelease?.latestVersion;
    if (!target) return Promise.resolve();
    return installVersion(target, true);
  }, [availableRelease?.latestVersion, installVersion]);

  const dismissPrompt = useCallback(() => {
    if (readyVersionRef.current) {
      dismissedVersionRef.current = readyVersionRef.current;
    }
    setPromptOpen(false);
  }, []);

  const syncExtensions = useCallback(async () => {
    if (syncing) return;
    setSyncing(true);
    try {
      const result = await syncOfficialExtensions();
      onToast(a.syncExtensionsSuccess(result.updatedSkills.length + result.updatedMcps.length));
    } catch (cause) {
      const detail = cause instanceof Error ? cause.message : String(cause);
      onError(a.updateAppFailed(detail));
    } finally {
      setSyncing(false);
    }
  }, [a, onToast, onError, syncing]);

  const updatingApp = step === "downloading" || step === "verifying" || step === "installing";

  return {
    checking,
    checkResult,
    availableRelease,
    step,
    updatingApp,
    progress,
    readyVersion,
    updatedTo,
    promptOpen,
    syncing,
    checkNow,
    updateApp,
    installReadyAndRestart,
    dismissPrompt,
    syncExtensions,
    restart,
  };
}
