import { useState } from "react";
import { useI18n } from "./useI18n";
import {
  batchSetMcpAll,
  coversHarness,
  deleteMcp,
  deployMcps,
  saveMcp,
  setMcpState,
  type McpServer,
} from "../lib/mcps";
import { GLOBAL_SCOPE } from "../lib/harnesses";

function toMessage(cause: unknown, fallback: string): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return fallback;
}

interface McpMutationsOptions {
  scope: string;
  scopeLabel: string;
  replaceServers: (updater: (items: McpServer[]) => McpServer[]) => void;
  load: (force?: boolean) => Promise<void>;
  setError: (message: string | null) => void;
  onToast: (message: string) => void;
  /** Closed by `save` on success; cleared together with the form error. */
  onSaved: () => void;
}

/**
 * All catalog writes behind the MCP registry: per-server toggles, bulk
 * projections, saves, deletes and recompiles. Every write is optimistic
 * with a catalog reload as the rollback; the Rust side re-compiles every
 * harness config before it resolves.
 */
export function useMcpMutations({
  scope,
  scopeLabel,
  replaceServers,
  load,
  setError,
  onToast,
  onSaved,
}: McpMutationsOptions) {
  const { t } = useI18n();
  const [busyIds, setBusyIds] = useState<ReadonlySet<string>>(() => new Set());
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [deploying, setDeploying] = useState(false);
  const [batchPending, setBatchPending] = useState(false);

  const setPending = (id: string, pending: boolean) =>
    setBusyIds((previous) => {
      const next = new Set(previous);
      if (pending) next.add(id);
      else next.delete(id);
      return next;
    });

  const toggle = async (server: McpServer) => {
    const harnessId = scope === GLOBAL_SCOPE.id ? null : scope;
    const enabled = !coversHarness(server, scope);

    replaceServers((previous) =>
      previous.map((item) => {
        if (item.id !== server.id) return item;
        if (harnessId === null) return { ...item, globalEnabled: enabled };
        return {
          ...item,
          enabledHarnesses: enabled
            ? [...new Set([...item.enabledHarnesses, harnessId])]
            : item.enabledHarnesses.filter((id) => id !== harnessId),
        };
      }),
    );
    setPending(server.id, true);
    setError(null);

    try {
      await setMcpState(server.id, harnessId, enabled);
      onToast(
        t.mcps.toggled(server.name, enabled ? t.mcps.enabledVerb : t.mcps.disabledVerb, scopeLabel),
      );
    } catch (cause) {
      setError(toMessage(cause, t.common.unexpected));
      await load();
    } finally {
      setPending(server.id, false);
    }
  };

  const applyAll = async (enabled: boolean, serverCount: number) => {
    if (serverCount === 0 || batchPending) return;
    const harnessId = scope === GLOBAL_SCOPE.id ? null : scope;

    // A harness scope never touches the Global baseline, mirroring Rust.
    replaceServers((previous) =>
      previous.map((item) => {
        if (harnessId === null) return { ...item, globalEnabled: enabled };
        return {
          ...item,
          enabledHarnesses: enabled
            ? [...new Set([...item.enabledHarnesses, harnessId])]
            : item.enabledHarnesses.filter((id) => id !== harnessId),
        };
      }),
    );
    setBatchPending(true);
    setError(null);

    try {
      await batchSetMcpAll(harnessId, enabled);
      onToast(t.mcps.batchApplied(enabled, scopeLabel));
    } catch (cause) {
      setError(t.mcps.batchFailed(toMessage(cause, t.common.unexpected)));
      await load();
    } finally {
      setBatchPending(false);
    }
  };

  const remove = async (server: McpServer) => {
    setPending(server.id, true);
    setError(null);

    try {
      await deleteMcp(server.id);
      replaceServers((previous) => previous.filter((item) => item.id !== server.id));
      onToast(t.mcps.deletedFromConfigs(server.name));
    } catch (cause) {
      setError(t.mcps.deleteFailed(server.name, toMessage(cause, t.common.unexpected)));
    } finally {
      setPending(server.id, false);
    }
  };

  const save = async (server: McpServer) => {
    setSaving(true);
    setFormError(null);

    try {
      await saveMcp(server);
      onSaved();
      onToast(t.mcps.savedDeployed(server.name));
      await load();
    } catch (cause) {
      setFormError(toMessage(cause, t.common.unexpected));
    } finally {
      setSaving(false);
    }
  };

  const recompile = async () => {
    setDeploying(true);
    setError(null);

    try {
      const changed = await deployMcps();
      onToast(changed.length === 0 ? t.mcps.alreadyMatches : t.mcps.recompiled(changed.length));
      await load();
    } catch (cause) {
      setError(t.mcps.recompileFailed(toMessage(cause, t.common.unexpected)));
    } finally {
      setDeploying(false);
    }
  };

  return {
    busyIds,
    saving,
    formError,
    setFormError,
    deploying,
    batchPending,
    toggle,
    applyAll,
    remove,
    save,
    recompile,
  };
}
