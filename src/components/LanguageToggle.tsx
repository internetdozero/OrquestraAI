import { Languages } from "lucide-react";
import { LOCALES, type Locale } from "../i18n/I18nContext";
import { useI18n } from "../hooks/useI18n";
import { cn } from "../lib/utils";

/** Short codes shown on the pill, independent of the active locale. */
const CODES: Record<Locale, string> = { pt: "PT", en: "EN" };

/**
 * Instant locale switcher: both dictionaries are already loaded, so a click
 * re-renders the tree with no reload, network request or flash of English.
 */
export function LanguageToggle() {
  const { t, locale, setLocale } = useI18n();

  return (
    <div
      role="group"
      aria-label={t.common.language}
      className="flex items-center gap-0.5 rounded-md border border-white/[0.07] bg-white/[0.03] p-0.5"
    >
      <Languages className="mx-1 size-3 text-zinc-500" aria-hidden />
      {LOCALES.map((option) => (
        <button
          key={option}
          type="button"
          onClick={() => setLocale(option)}
          aria-pressed={option === locale}
          title={t.common.languageNames[option]}
          className={cn(
            "rounded px-1.5 py-0.5 font-mono text-[10px] font-medium transition-colors focus-visible:ring-2 focus-visible:ring-emerald-500/40 focus-visible:outline-none",
            option === locale
              ? "bg-emerald-500/15 text-emerald-300"
              : "text-zinc-500 hover:text-zinc-300",
          )}
        >
          {CODES[option]}
        </button>
      ))}
    </div>
  );
}
