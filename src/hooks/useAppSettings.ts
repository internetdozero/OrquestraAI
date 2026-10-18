import { useCallback, useEffect, useState } from "react";
import type { Locale } from "../i18n/I18nContext";
import { useI18n } from "./useI18n";

export type AccentColor = "emerald" | "cyan" | "violet" | "amber";

export interface AppSettings {
  minimizeToTray: boolean;
  notifyOnPurge: boolean;
  ramAlertThreshold: number;
  tmpWatchEnabled: boolean;
  accentColor: AccentColor;
  autoCheckAppUpdates: boolean;
  autoCheckExtensionUpdates: boolean;
}

const STORAGE_KEY = "orquestra_settings";

export const DEFAULT_SETTINGS: AppSettings = {
  minimizeToTray: true,
  notifyOnPurge: true,
  ramAlertThreshold: 85,
  tmpWatchEnabled: true,
  accentColor: "emerald",
  autoCheckAppUpdates: true,
  autoCheckExtensionUpdates: true,
};

function clampRam(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_SETTINGS.ramAlertThreshold;
  return Math.min(95, Math.max(70, Math.round(value)));
}

function readStored(): Partial<AppSettings> {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Partial<AppSettings>;
    return typeof parsed === "object" && parsed !== null ? parsed : {};
  } catch {
    return {};
  }
}

/**
 * Desktop preferences persisted in `localStorage`. Language lives in
 * `I18nContext` and is mirrored here through `locale`/`setLocale` so the
 * General tab reads and writes a single source of truth.
 */
export function useAppSettings() {
  const { locale, setLocale } = useI18n();
  const [settings, setSettings] = useState<AppSettings>(() => ({
    ...DEFAULT_SETTINGS,
    ...readStored(),
    ramAlertThreshold: clampRam(
      readStored().ramAlertThreshold ?? DEFAULT_SETTINGS.ramAlertThreshold,
    ),
  }));

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    } catch {
      // Persistence is a nicety; the in-memory settings still apply.
    }
  }, [settings]);

  // Live accent theme: the CSS palettes in `index.css` react to this
  // attribute, so active/brand touchpoints follow the setting instantly.
  useEffect(() => {
    document.documentElement.dataset.accent = settings.accentColor;
  }, [settings.accentColor]);

  const updateSetting = useCallback(
    <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
      setSettings((prev) => ({
        ...prev,
        [key]: key === "ramAlertThreshold" ? clampRam(value as number) : value,
      }));
    },
    [],
  );

  const setLanguage = useCallback(
    (next: Locale) => setLocale(next),
    [setLocale],
  );

  const setAutoCheckAppUpdates = useCallback((enabled: boolean) => {
    setSettings((prev) => ({ ...prev, autoCheckAppUpdates: enabled }));
  }, []);

  const setAutoCheckExtensionUpdates = useCallback((enabled: boolean) => {
    setSettings((prev) => ({ ...prev, autoCheckExtensionUpdates: enabled }));
  }, []);

  const resetDefaults = useCallback(() => {
    setSettings(DEFAULT_SETTINGS);
  }, []);

  return {
    settings,
    updateSetting,
    setAutoCheckAppUpdates,
    setAutoCheckExtensionUpdates,
    resetDefaults,
    locale,
    setLanguage,
  };
}

export type AppSettingsState = ReturnType<typeof useAppSettings>;
