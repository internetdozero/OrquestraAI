import { useCallback, useEffect, useState } from "react";
import type { Locale } from "../i18n/I18nContext";
import { useI18n } from "./useI18n";

export type AccentColor = "emerald" | "cyan" | "violet" | "amber" | "silver";

export interface AppSettings {
  minimizeToTray: boolean;
  notifyOnPurge: boolean;
  ramAlertThreshold: number;
  tmpWatchEnabled: boolean;
  accentColor: AccentColor;
  autoCheckAppUpdates: boolean;
  autoCheckExtensionUpdates: boolean;
}

const STORAGE_KEY = "ownhub_settings";
/** Stamped after the first read so the one-time accent upgrade runs once. */
const SCHEMA_KEY = "ownhub_settings_schema";
const SCHEMA_VERSION = "2";

export const DEFAULT_SETTINGS: AppSettings = {
  minimizeToTray: true,
  notifyOnPurge: true,
  ramAlertThreshold: 85,
  tmpWatchEnabled: true,
  accentColor: "silver",
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
    if (typeof parsed !== "object" || parsed === null) return {};
    return upgraded(parsed);
  } catch {
    return {};
  }
}

/** Set once the upgrade has run in this session, so the marker it writes cannot
 * make a second read (React re-invokes the state initializer) undo the result. */
let upgradedThisSession = false;

/**
 * One-time schema upgrade. Every install persists the whole settings object on
 * first mount, so a stored `violet` is indistinguishable from the old default
 * — schema 1 wrote it for everyone. Dropping it lets the new default (silver)
 * apply; the schema marker keeps a later, deliberate violet choice intact.
 */
function upgraded(stored: Partial<AppSettings>): Partial<AppSettings> {
  if (upgradedThisSession) return stored;
  upgradedThisSession = true;

  let firstRun = false;
  try {
    firstRun = window.localStorage.getItem(SCHEMA_KEY) !== SCHEMA_VERSION;
    if (firstRun) window.localStorage.setItem(SCHEMA_KEY, SCHEMA_VERSION);
  } catch {
    // Storage refused the marker: keep the stored choice rather than fight it.
    return stored;
  }
  if (!firstRun || stored.accentColor !== "violet") return stored;
  // Omit the key entirely: an explicit `undefined` would win over the default
  // in the spread below and leave `data-accent` unset.
  const rest: Partial<AppSettings> = { ...stored };
  delete rest.accentColor;
  return rest;
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

  const setLanguage = useCallback((next: Locale) => setLocale(next), [setLocale]);

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
