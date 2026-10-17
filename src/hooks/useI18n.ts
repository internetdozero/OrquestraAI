import { useI18nContext } from "../i18n/I18nContext";

/**
 * Translation access for components: `const { t, locale, setLocale } = useI18n()`.
 * `t` is the fully typed dictionary, so `t.sidebar.workspace` is checked at
 * build time and a typo cannot reach production.
 */
export function useI18n() {
  return useI18nContext();
}
