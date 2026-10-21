import { Cpu, Globe, ShieldAlert, Sparkles, Zap } from "lucide-react";
import { cn } from "../lib/utils";

const AVAILABLE_ICONS = [
  { key: "shield", icon: ShieldAlert, label: "Shield" },
  { key: "cpu", icon: Cpu, label: "CPU" },
  { key: "globe", icon: Globe, label: "Globe" },
  { key: "zap", icon: Zap, label: "Zap" },
  { key: "custom", icon: Sparkles, label: "Sparkles" },
];

interface PresetIconPickerProps {
  label: string;
  value: string;
  onChange: (key: string) => void;
}

/** Preset glyph selector. */
export function PresetIconPicker({ label, value, onChange }: PresetIconPickerProps) {
  return (
    <div>
      <label className="block text-[10px] font-medium tracking-[0.12em] text-zinc-400 uppercase">
        {label}
      </label>
      <div className="mt-1.5 flex gap-2">
        {AVAILABLE_ICONS.map((item) => {
          const IconComp = item.icon;
          const isSelected = value === item.key;
          return (
            <button
              key={item.key}
              type="button"
              onClick={() => onChange(item.key)}
              className={cn(
                "flex size-9 items-center justify-center rounded-lg border transition-colors",
                isSelected
                  ? "border-emerald-500/50 bg-emerald-500/20 text-emerald-300"
                  : "border-white/10 bg-white/[0.02] text-zinc-400 hover:border-white/20 hover:text-zinc-200",
              )}
              title={item.label}
            >
              <IconComp className="size-4" />
            </button>
          );
        })}
      </div>
    </div>
  );
}
