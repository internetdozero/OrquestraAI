import { useEffect, useRef } from "react";
import { Unplug } from "lucide-react";
import { useAgentsFile } from "../hooks/useAgentsFile";
import { useI18n } from "../hooks/useI18n";
import { AgentsEditor } from "./AgentsEditor";
import { ErrorBanner, StatePanel } from "./ui";

interface AgentsViewProps {
  isTauri: boolean;
  /** Shell-level error (bridge, IPC) shown above the editor. */
  error: string | null;
  onToast: (message: string) => void;
  onError: (message: string) => void;
  /** Clears the shell's stale error once the file round-trips cleanly. */
  onClear: () => void;
  /** Shell refresh button bumps this to re-read the file from disk. */
  reloadToken?: number;
}

/** Route for the master `~/.orquestra/AGENTS.md`, loaded and saved over IPC. */
export function AgentsView({
  isTauri,
  error,
  onToast,
  onError,
  onClear,
  reloadToken = 0,
}: AgentsViewProps) {
  const { t } = useI18n();
  const agents = useAgentsFile({ isTauri, onToast, onError, onClear });
  const { load } = agents;

  useEffect(() => {
    if (isTauri) void load();
  }, [isTauri, load]);

  // The shell asks for a re-read by bumping the token; skip the mount value.
  const lastReloadToken = useRef(reloadToken);
  useEffect(() => {
    if (!isTauri || reloadToken === lastReloadToken.current) return;
    lastReloadToken.current = reloadToken;
    void load();
  }, [isTauri, reloadToken, load]);

  if (!isTauri) {
    return (
      <div className="flex h-full min-h-0 flex-col">
        {error && <ErrorBanner message={error} onDismiss={onClear} />}
        <div className="flex h-full items-center justify-center">
          <StatePanel
            icon={Unplug}
            title={t.skills.stateUnavailableTitle}
            description={t.agents.unavailableBody}
          >
            <code className="rounded border border-white/10 bg-white/[0.03] px-2 py-1 font-mono text-[10.5px] text-zinc-400">
              npm run tauri dev
            </code>
          </StatePanel>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      {error && <ErrorBanner message={error} onDismiss={onClear} />}
      <div className="min-h-0 flex-1">
        <AgentsEditor
          content={agents.content}
          savedContent={agents.savedContent}
          loading={agents.loading}
          saving={agents.saving}
          onChange={agents.setContent}
          onSave={() => void agents.save()}
        />
      </div>
    </div>
  );
}
