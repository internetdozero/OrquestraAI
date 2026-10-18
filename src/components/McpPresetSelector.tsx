import { MCP_PRESETS, type McpPreset } from "../lib/mcpPresets";

/** Quick-select chips that pre-fill the whole MCP form instantly. */
export function McpPresetSelector({ onPick }: { onPick: (preset: McpPreset) => void }) {
  return (
    <div className="flex flex-wrap gap-1.5" role="group" aria-label="MCP presets">
      {MCP_PRESETS.map((preset) => (
        <button
          key={preset.id}
          type="button"
          onClick={() => onPick(preset)}
          title={`${preset.name} — ${preset.description}`}
          className="flex items-center gap-1.5 rounded-full border border-white/10 bg-white/[0.03] px-2.5 py-1 text-[11px] font-medium text-zinc-300 transition hover:border-emerald-500/40 hover:text-zinc-100"
        >
          <span aria-hidden>{preset.emoji}</span>
          {preset.name}
        </button>
      ))}
    </div>
  );
}
