import { useEffect, useState } from "react";
import {
  Database,
  Info,
  RotateCcw,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  X,
} from "lucide-react";
import { useI18n } from "../../hooks/useI18n";
import type { AppSettingsState } from "../../hooks/useAppSettings";
import type { CoreStatus } from "../../lib/tauri";
import { cn } from "../../lib/utils";
import { AboutSettingsTab } from "./AboutSettingsTab";
import { GeneralSettingsTab } from "./GeneralSettingsTab";
import { JanitorSettingsTab } from "./JanitorSettingsTab";
import { RetentionSettingsTab } from "./RetentionSettingsTab";
import { StorageSettingsTab } from "./StorageSettingsTab";

type SettingsTabId = "general" | "storage" | "retention" | "janitor" | "about";

const TABS: { id: SettingsTabId; icon: typeof Settings2 }[] = [
  { id: "general", icon: Sparkles },
  { id: "storage", icon: Database },
  { id: "retention", icon: ShieldCheck },
  { id: "janitor", icon: SlidersHorizontal },
  { id: "about", icon: Info },
];

interface SettingsModalProps {
  prefs: AppSettingsState;
  status: CoreStatus | null;
  isTauri: boolean;
  initialTab?: SettingsTabId;
  onClose: () => void;
  onToast: (message: string) => void;
  onError: (message: string) => void;
  onOpenSessions: () => void;
}

/** Linear-style dialog: left tab rail, right content, reset in the footer. */
export function SettingsModal({
  prefs,
  status,
  isTauri,
  initialTab = "general",
  onClose,
  onToast,
  onError,
  onOpenSessions,
}: SettingsModalProps) {
  const { t } = useI18n();
  const s = t.settings;
  const [tab, setTab] = useState<SettingsTabId>(initialTab);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        role="dialog"
        aria-modal="true"
        aria-label={s.title}
        className="flex h-[560px] max-h-[88vh] w-full max-w-2xl flex-col overflow-hidden rounded-2xl border border-white/10 bg-[#0e0e12] shadow-[0_30px_80px_-20px_rgba(0,0,0,0.9)]"
      >
        <header className="flex items-start justify-between gap-4 border-b border-white/[0.06] px-5 py-4">
          <div className="flex items-center gap-3">
            <div className="grid size-9 place-items-center rounded-lg border border-white/10 bg-white/[0.03] text-zinc-200">
              <Settings2 className="size-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-zinc-100">{s.title}</h2>
              <p className="text-[11px] text-zinc-500">{s.subtitle}</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={s.close}
            className="rounded-md p-1.5 text-zinc-500 transition hover:bg-white/[0.06] hover:text-zinc-200"
          >
            <X className="size-4" />
          </button>
        </header>

        <div className="flex min-h-0 flex-1">
          <nav
            aria-label={s.title}
            className="w-36 shrink-0 space-y-0.5 border-r border-white/[0.06] bg-white/[0.01] p-2"
          >
            {TABS.map((entry) => {
              const Icon = entry.icon;
              const active = tab === entry.id;
              return (
                <button
                  key={entry.id}
                  type="button"
                  onClick={() => setTab(entry.id)}
                  aria-current={active ? "page" : undefined}
                  className={cn(
                    "flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-[12px] font-medium transition-[background-color,color]",
                    active
                      ? "accent-bg-soft accent-text"
                      : "text-zinc-500 hover:bg-white/[0.04] hover:text-zinc-200",
                  )}
                >
                  <Icon className="size-3.5 shrink-0" />
                  {s.tabs[entry.id]}
                </button>
              );
            })}
          </nav>

          <div className="scroll-pane min-h-0 flex-1 overflow-y-auto px-5 py-4">
            {tab === "general" && (
              <GeneralSettingsTab
                locale={prefs.locale}
                onLanguage={prefs.setLanguage}
                minimizeToTray={prefs.settings.minimizeToTray}
                notifyOnPurge={prefs.settings.notifyOnPurge}
                accentColor={prefs.settings.accentColor}
                onToggleMinimize={() =>
                  prefs.updateSetting("minimizeToTray", !prefs.settings.minimizeToTray)
                }
                onToggleNotify={() =>
                  prefs.updateSetting("notifyOnPurge", !prefs.settings.notifyOnPurge)
                }
                onAccent={(accent) => prefs.updateSetting("accentColor", accent)}
              />
            )}
            {tab === "storage" && (
              <StorageSettingsTab isTauri={isTauri} onToast={onToast} onError={onError} />
            )}
            {tab === "retention" && (
              <RetentionSettingsTab
                isTauri={isTauri}
                onToast={onToast}
                onError={onError}
                onOpenSessions={onOpenSessions}
              />
            )}
            {tab === "janitor" && (
              <JanitorSettingsTab
                ramAlertThreshold={prefs.settings.ramAlertThreshold}
                tmpWatchEnabled={prefs.settings.tmpWatchEnabled}
                onRamThreshold={(value) => prefs.updateSetting("ramAlertThreshold", value)}
                onToggleTmpWatch={() =>
                  prefs.updateSetting("tmpWatchEnabled", !prefs.settings.tmpWatchEnabled)
                }
              />
            )}
            {tab === "about" && (
              <AboutSettingsTab
                status={status}
                isTauri={isTauri}
                autoCheckAppUpdates={prefs.settings.autoCheckAppUpdates}
                autoCheckExtensionUpdates={prefs.settings.autoCheckExtensionUpdates}
                onToggleAppUpdates={() =>
                  prefs.setAutoCheckAppUpdates(!prefs.settings.autoCheckAppUpdates)
                }
                onToggleExtensionUpdates={() =>
                  prefs.setAutoCheckExtensionUpdates(
                    !prefs.settings.autoCheckExtensionUpdates,
                  )
                }
                onToast={onToast}
                onError={onError}
              />
            )}
          </div>
        </div>

        <footer className="flex items-center justify-between gap-3 border-t border-white/[0.06] bg-white/[0.015] px-5 py-2.5">
          <p className="font-mono text-[10px] text-zinc-600">~/.orquestra · v0.1.0</p>
          <button
            type="button"
            onClick={() => {
              prefs.resetDefaults();
              onToast(s.resetDone);
            }}
            className="flex items-center gap-1.5 rounded-md border border-white/10 bg-white/[0.02] px-2.5 py-1 text-[11px] font-medium text-zinc-400 transition hover:text-zinc-200"
          >
            <RotateCcw className="size-3" />
            {s.reset}
          </button>
        </footer>
      </section>
    </div>
  );
}
