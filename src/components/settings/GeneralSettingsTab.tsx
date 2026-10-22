import { Globe } from "lucide-react";
import { LOCALES, type Locale } from "../../i18n/I18nContext";
import { useI18n } from "../../hooks/useI18n";
import type { AccentColor } from "../../hooks/useAppSettings";
import { cn } from "../../lib/utils";

// Swatch colors live as [data-swatch] rules in index.css: style-src carries
// no 'unsafe-inline', so the finite palette maps to static CSS.
const ACCENTS: { id: AccentColor }[] = [
  { id: "emerald" },
  { id: "cyan" },
  { id: "violet" },
  { id: "amber" },
];

function Toggle({
  checked,
  onFlip,
  label,
}: {
  checked: boolean;
  onFlip: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={onFlip}
      className={cn(
        "relative h-5.5 w-10 shrink-0 rounded-full transition-[background-color]",
        checked ? "accent-dot" : "bg-zinc-700",
      )}
    >
      <span
        className={cn(
          "absolute top-0.5 size-4.5 rounded-full bg-white transition-[left]",
          checked ? "left-5" : "left-0.5",
        )}
      />
    </button>
  );
}

interface GeneralSettingsTabProps {
  locale: Locale;
  onLanguage: (locale: Locale) => void;
  minimizeToTray: boolean;
  notifyOnPurge: boolean;
  accentColor: AccentColor;
  onToggleMinimize: () => void;
  onToggleNotify: () => void;
  onAccent: (accent: AccentColor) => void;
}

/** Language, window behavior and accent theme. */
export function GeneralSettingsTab({
  locale,
  onLanguage,
  minimizeToTray,
  notifyOnPurge,
  accentColor,
  onToggleMinimize,
  onToggleNotify,
  onAccent,
}: GeneralSettingsTabProps) {
  const { t } = useI18n();
  const g = t.settings.general;

  return (
    <div className="space-y-5">
      <section>
        <h3 className="flex items-center gap-1.5 text-[12.5px] font-semibold text-zinc-100">
          <Globe className="accent-text size-3.5" />
          {g.languageTitle}
        </h3>
        <p className="mt-0.5 text-[11px] text-zinc-500">{g.languageHint}</p>
        <div role="radiogroup" aria-label={g.languageTitle} className="mt-2 grid grid-cols-2 gap-2">
          {LOCALES.map((option) => (
            <button
              key={option}
              type="button"
              role="radio"
              aria-checked={option === locale}
              onClick={() => onLanguage(option)}
              className={cn(
                "rounded-lg border px-3 py-2 text-left text-[12px] font-medium transition-[border-color,background-color]",
                option === locale
                  ? "accent-border accent-bg-soft accent-text"
                  : "border-white/10 bg-white/[0.02] text-zinc-400 hover:border-white/20 hover:text-zinc-200",
              )}
            >
              {t.common.languageNames[option]}
            </button>
          ))}
        </div>
      </section>

      <section>
        <h3 className="text-[12.5px] font-semibold text-zinc-100">{g.windowTitle}</h3>
        <div className="mt-2 space-y-2">
          {[
            {
              title: g.minimizeToTray,
              hint: g.minimizeToTrayHint,
              checked: minimizeToTray,
              onFlip: onToggleMinimize,
            },
            {
              title: g.notifyOnPurge,
              hint: g.notifyOnPurgeHint,
              checked: notifyOnPurge,
              onFlip: onToggleNotify,
            },
          ].map((row) => (
            <div
              key={row.title}
              className="flex items-center justify-between gap-3 rounded-lg border border-white/[0.07] bg-white/[0.02] px-3 py-2.5"
            >
              <div>
                <p className="text-[12.5px] text-zinc-200">{row.title}</p>
                <p className="text-[11px] text-zinc-500">{row.hint}</p>
              </div>
              <Toggle checked={row.checked} onFlip={row.onFlip} label={row.title} />
            </div>
          ))}
        </div>
      </section>

      <section>
        <h3 className="text-[12.5px] font-semibold text-zinc-100">{g.accentTitle}</h3>
        <p className="mt-0.5 text-[11px] text-zinc-500">{g.accentHint}</p>
        <div className="mt-2 flex gap-2">
          {ACCENTS.map((accent) => (
            <button
              key={accent.id}
              type="button"
              onClick={() => onAccent(accent.id)}
              aria-pressed={accent.id === accentColor}
              title={g.accents[accent.id]}
              className={cn(
                "flex items-center gap-2 rounded-lg border px-2.5 py-1.5 text-[11.5px] transition-[border-color,background-color]",
                accent.id === accentColor
                  ? "accent-border accent-bg-soft text-zinc-100"
                  : "border-white/10 bg-white/[0.02] text-zinc-400 hover:text-zinc-200",
              )}
            >
              <span data-swatch={accent.id} className="size-3.5 rounded-full" />
              {g.accents[accent.id]}
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}
