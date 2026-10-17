import type { TranslationSchema } from "./locales/pt";

/** Rust ships these sentences verbatim; the UI translates the known set. */
export function bridgeRationale(t: TranslationSchema, raw: string): string {
  switch (raw) {
    case "Shared user root that Codex and Grok inject into every session":
      return t.bridge.rationales.sharedRoot;
    case "Vendor built-ins ship with the app; archived in the vault so ~/.gemini/config/skills stays the only user source":
      return t.bridge.rationales.vendorBuiltins;
    case "Curated vendor checkout Codex indexes beside ~/.codex/skills":
      return t.bridge.rationales.codexVendor;
    case "Bundled marketplace cache of plugin skills":
      return t.bridge.rationales.bundledMarket;
    case "Stale marketplace cache of plugin skills":
      return t.bridge.rationales.staleMarket;
    case "Plugin marketplace skills load beside ~/.claude/skills":
      return t.bridge.rationales.claudePlugins;
    case "Optional skill repository Hermes auto-loads beside ~/.hermes/skills":
      return t.bridge.rationales.hermesOptional;
    case "Grok resolves its built-ins here and ignores ~/.grok/skills":
      return t.bridge.rationales.grokBundled;
    case "Third-party plugin bundles Grok loads on startup":
      return t.bridge.rationales.grokPlugins;
    default:
      return raw;
  }
}

/** Same treatment for the short harness notes on the link plan. */
export function bridgeNote(t: TranslationSchema, raw: string): string {
  switch (raw) {
    case "Internal .system folders are moved into the projection first":
      return t.bridge.notes.codexSystem;
    case "Manicode shares the Freebuff projection":
      return t.bridge.notes.freebuff;
    case "Curator state files are kept inside the projection":
      return t.bridge.notes.hermesCurator;
    default:
      return raw;
  }
}
