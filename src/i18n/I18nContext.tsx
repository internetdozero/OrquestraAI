import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import en from "./locales/en";
import pt, { type TranslationSchema } from "./locales/pt";

/** Active interface language. */
export type Locale = "pt" | "en";

/** Every locale the switcher offers, in display order. */
export const LOCALES: Locale[] = ["pt", "en"];

const STORAGE_KEY = "orquestra_locale";

const DICTIONARIES: Record<Locale, TranslationSchema> = { pt, en };

interface I18nValue {
  /** Dictionary for the active locale. */
  t: TranslationSchema;
  locale: Locale;
  setLocale: (locale: Locale) => void;
}

const I18nContext = createContext<I18nValue | null>(null);

function isLocale(value: string | null): value is Locale {
  return value === "pt" || value === "en";
}

/** A saved choice wins; otherwise a `pt*` navigator falls back to Portuguese. */
function detectLocale(): Locale {
  if (typeof window !== "undefined") {
    try {
      const stored = window.localStorage.getItem(STORAGE_KEY);
      if (isLocale(stored)) return stored;
    } catch {
      // Hardened webviews can deny storage access; detection simply continues.
    }
  }

  const language = typeof navigator === "undefined" ? "" : navigator.language;
  return language.toLowerCase().startsWith("pt") ? "pt" : "en";
}

/**
 * Provides the active dictionary and switching to the whole tree. Both
 * dictionaries are tiny static objects, so switching re-renders without any
 * network or async work.
 */
export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(detectLocale);

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    try {
      window.localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Persisting the preference is a nicety, never a requirement.
    }
  }, []);

  useEffect(() => {
    document.documentElement.lang = locale === "pt" ? "pt-BR" : "en";
  }, [locale]);

  const value = useMemo<I18nValue>(
    () => ({ t: DICTIONARIES[locale], locale, setLocale }),
    [locale, setLocale],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

/** Context accessor; `useI18n` is the hook components should use. */
export function useI18nContext(): I18nValue {
  const value = useContext(I18nContext);
  if (!value) {
    throw new Error("useI18n must be used inside an <I18nProvider>");
  }
  return value;
}
