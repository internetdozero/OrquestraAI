import { useCallback, useState } from "react";
import { useI18n } from "./useI18n";
import { readMasterAgentsMd, writeMasterAgentsMd } from "../lib/tauri";

/** Draft and save state for the master `~/.orquestra/AGENTS.md`. */
export function useAgentsFile(options: {
  isTauri: boolean;
  onToast: (message: string) => void;
  onError: (message: string) => void;
  /** Clears any stale shell error once the file round-trips cleanly. */
  onClear: () => void;
}) {
  const { isTauri, onToast, onError, onClear } = options;
  const { t } = useI18n();
  const [content, setContent] = useState("");
  const [savedContent, setSavedContent] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    if (!isTauri) return;

    setLoading(true);
    try {
      const text = await readMasterAgentsMd();
      setContent(text);
      setSavedContent(text);
      onClear();
    } catch (cause) {
      onError(t.agents.loadFailed(message(cause, t.common.unexpected)));
    } finally {
      setLoading(false);
    }
  }, [isTauri, onError, onClear, t]);

  const save = useCallback(async () => {
    if (!isTauri || saving || content === savedContent) return;

    setSaving(true);
    try {
      await writeMasterAgentsMd(content);
      setSavedContent(content);
      onClear();
      onToast(t.agents.savedToast);
    } catch (cause) {
      onError(t.agents.saveFailed(message(cause, t.common.unexpected)));
    } finally {
      setSaving(false);
    }
  }, [isTauri, saving, content, savedContent, onToast, onError, onClear, t]);

  const dirty = content !== savedContent;

  return { content, savedContent, loading, saving, dirty, setContent, load, save };
}

function message(cause: unknown, fallback: string): string {
  if (typeof cause === "string") return cause;
  if (cause instanceof Error) return cause.message;
  return fallback;
}
