import { useCallback, useEffect, useState } from "react";
import { useI18n } from "./useI18n";
import {
  backupName,
  deployHarnessBridge,
  getBridgeStatus,
  type HarnessStatus,
} from "../lib/bridge";

/**
 * Live harness links plus the fusion deploy. `onDeployed` runs after a
 * successful deploy so the shell can re-read whatever fusion invalidated.
 */
export function useHarnessBridge(options: {
  isTauri: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
  onDeployed: () => void;
}) {
  const { isTauri, onToast, onError, onDeployed } = options;
  const { t } = useI18n();
  const [statuses, setStatuses] = useState<HarnessStatus[]>([]);
  const [deploying, setDeploying] = useState(false);
  const [open, setOpen] = useState(false);

  const load = useCallback(async () => {
    if (!isTauri) return;

    try {
      setStatuses(await getBridgeStatus());
    } catch (cause) {
      onError(t.bridge.readFailed(message(cause, t.common.unexpected)));
    }
  }, [isTauri, onError, t]);

  useEffect(() => {
    void load();
  }, [load]);

  const deploy = useCallback(async () => {
    if (!isTauri || deploying) return;

    setDeploying(true);
    try {
      const report = await deployHarnessBridge();
      setStatuses(report.harnesses);
      setOpen(false);

      if (report.errors.length > 0) {
        onError(t.bridge.partial(report.errors.join("; ")));
      } else {
        const details: string[] = [];
        const backup = backupName(report.backupDir);
        if (backup) details.push(t.bridge.backupDetail(backup));
        if (report.adopted.length > 0) {
          details.push(t.bridge.adopted(report.adopted.length));
        }
        if (report.instructionsImported.length > 0) {
          details.push(t.bridge.imported(report.instructionsImported.join(", ")));
        }

        const extra = details.length > 0 ? ` · ${details.join(" · ")}` : "";
        onToast(t.bridge.connected(report.bridged, report.total, extra));
      }

      onDeployed();
    } catch (cause) {
      onError(t.bridge.deployFailed(message(cause, t.common.unexpected)));
    } finally {
      setDeploying(false);
    }
  }, [isTauri, deploying, onToast, onError, onDeployed, t]);

  return { statuses, deploying, open, setOpen, deploy };
}

function message(cause: unknown, fallback: string): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return fallback;
}
