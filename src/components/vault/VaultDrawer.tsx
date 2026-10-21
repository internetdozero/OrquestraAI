import { useEffect, useState } from "react";
import {
  listVaultPresets,
  vaultDeleteService,
  vaultListServices,
  vaultSaveService,
  vaultSetPrimary,
  type VaultPreset,
  type VaultService,
} from "../../lib/vault.ts";
import { confirmOnEnter } from "../../lib/utils.ts";
import { CustomDropdown } from "../CustomDropdown.tsx";

interface VaultDrawerProps {
  open: boolean;
  onClose: () => void;
}

function healthBadge(health: string): string {
  switch (health) {
    case "Healthy":
      return "bg-emerald-500/15 text-emerald-300 border-emerald-500/30";
    case "Degraded":
      return "bg-amber-500/15 text-amber-300 border-amber-500/30";
    case "RateLimited":
      return "bg-orange-500/15 text-orange-300 border-orange-500/30";
    case "QuotaExceeded":
      return "bg-red-500/15 text-red-300 border-red-500/30";
    case "InvalidKey":
      return "bg-red-500/15 text-red-300 border-red-500/30";
    default:
      return "bg-zinc-500/15 text-zinc-300 border-zinc-500/30";
  }
}

export default function VaultDrawer({ open, onClose }: VaultDrawerProps) {
  const [services, setServices] = useState<VaultService[]>([]);
  const [presets, setPresets] = useState<VaultPreset[]>([]);
  const [presetId, setPresetId] = useState("github");
  const [serviceId, setServiceId] = useState("");
  const [label, setLabel] = useState("");
  const [keysText, setKeysText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    listVaultPresets()
      .then(setPresets)
      .catch(() => setPresets([]));
    vaultListServices()
      .then(setServices)
      .catch(() => setServices([]));
  }, [open]);

  if (!open) return null;

  async function refresh(): Promise<void> {
    setServices(await vaultListServices());
  }

  async function handleSave(): Promise<void> {
    setError(null);
    setSaving(true);
    try {
      const keys = keysText
        .split("\n")
        .map((key) => key.trim())
        .filter(Boolean);
      await vaultSaveService({
        id: serviceId.trim(),
        label: label.trim() || serviceId.trim(),
        preset: presetId,
        keys,
      });
      setServiceId("");
      setLabel("");
      setKeysText("");
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div
      className="fixed inset-y-0 right-0 z-50 flex w-full max-w-md flex-col border-l border-white/10 bg-zinc-950/95 p-5 backdrop-blur"
      onKeyDown={(event) => confirmOnEnter(event, () => void handleSave())}
    >
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-zinc-100">Blind Credential Vault</h2>
        <button
          onClick={onClose}
          className="rounded px-2 py-1 text-sm text-zinc-400 hover:bg-white/10"
        >
          Close
        </button>
      </div>
      <p className="mb-4 text-xs text-zinc-400">
        Agents never see raw keys. The backend injects the active key in memory for HTTP and exec
        calls.
      </p>

      <span className="mb-1 block text-xs text-zinc-400">Preset</span>
      <CustomDropdown
        value={presetId}
        ariaLabel="Preset"
        options={presets.map((preset) => ({ id: preset.id, label: preset.label }))}
        onChange={setPresetId}
        className="mb-3 w-full [&>button]:w-full [&>button]:justify-between [&>button]:py-2 [&>button]:text-[13px]"
      />

      <input
        value={serviceId}
        onChange={(event) => setServiceId(event.target.value)}
        placeholder="Service id (e.g. github-main)"
        className="mb-2 rounded border border-white/10 bg-zinc-900 px-2 py-1.5 text-sm text-zinc-100"
      />
      <input
        value={label}
        onChange={(event) => setLabel(event.target.value)}
        placeholder="Label (optional)"
        className="mb-2 rounded border border-white/10 bg-zinc-900 px-2 py-1.5 text-sm text-zinc-100"
      />
      <textarea
        value={keysText}
        onChange={(event) => setKeysText(event.target.value)}
        placeholder="One key per line (primary first)"
        rows={3}
        className="mb-2 rounded border border-white/10 bg-zinc-900 px-2 py-1.5 text-sm text-zinc-100"
      />
      {error ? <p className="mb-2 text-xs text-red-300">{error}</p> : null}
      <button
        onClick={handleSave}
        disabled={saving || !serviceId.trim() || !keysText.trim()}
        className="mb-5 rounded bg-emerald-600 px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
      >
        {saving ? "Saving..." : "Save service"}
      </button>

      <div className="flex-1 space-y-2 overflow-y-auto">
        {services.map((service) => (
          <div key={service.id} className="rounded border border-white/10 bg-zinc-900/60 p-3">
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium text-zinc-100">{service.label}</span>
              <span
                className={`rounded border px-1.5 py-0.5 text-[11px] ${healthBadge(service.health)}`}
              >
                {service.health}
              </span>
            </div>
            <p className="mt-1 text-[11px] text-zinc-500">
              {service.id} · {service.preset} · {service.totalKeys} key(s) · active #
              {service.activeKeyIndex}
            </p>
            <div className="mt-2 space-y-1">
              {service.maskedKeys.map((masked, index) => (
                <div
                  key={`${service.id}-${index}`}
                  className="flex items-center justify-between text-xs"
                >
                  <code className="text-zinc-300">{masked}</code>
                  {index === service.activeKeyIndex ? (
                    <span className="text-emerald-300">primary</span>
                  ) : (
                    <button
                      onClick={() =>
                        vaultSetPrimary(service.id, index)
                          .then(refresh)
                          .catch(() => undefined)
                      }
                      className="text-zinc-400 underline hover:text-zinc-200"
                    >
                      make primary
                    </button>
                  )}
                </div>
              ))}
            </div>
            <button
              onClick={() =>
                vaultDeleteService(service.id)
                  .then(refresh)
                  .catch(() => undefined)
              }
              className="mt-2 text-xs text-red-300 hover:text-red-200"
            >
              Remove
            </button>
          </div>
        ))}
        {services.length === 0 ? (
          <p className="text-xs text-zinc-500">No services yet. Add your first key pool above.</p>
        ) : null}
      </div>
    </div>
  );
}
