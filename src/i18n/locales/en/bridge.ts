import type { TranslationSchema } from "../pt";

/** English mirror of `pt/bridge.ts`. A missing or extra key fails the build. */
export const bridge: TranslationSchema["bridge"] = {
  projected: "projected",
  quarantined: "quarantined",
  archived: "archived",
  leaking: "leaking",
  notInstalled: "Not installed",
  manual: "Manual",
  linked: "Linked",
  leakingSkills: "Leaking skills",
  unlinked: "Unlinked",
  untouched: "Harness is not installed — the bridge leaves it untouched.",
  title: "Connect the Harness Bridge",
  summary: (managed: number, linked: number, pending: number) => {
    const found = `${managed} installed harness${managed === 1 ? "" : "es"} detected`;
    const done = `${linked} already linked`;
    const waiting = pending > 0 ? ` · ${pending} waiting` : "";
    return `${found} · ${done}${waiting}`;
  },
  warnLead:
    "Every skill folder, instruction file, and config file listed below is moved into",
  warnTail: "before a symlink is created. Nothing is deleted.",
  warnSees:
    "After linking, a harness only sees the skills you enabled for it in this app. Skills that exist only inside a harness are adopted into the vault first.",
  warnSecondary:
    "Secondary discovery locations — vendor checkouts, plugin marketplaces, and bundled skill repositories — are archived into the vault, then emptied or replaced with the projection link so no skill loads behind your back.",
  linkPlan: "Link plan",
  masterLabel: "master AGENTS.md · ~/.orquestra/AGENTS.md",
  reversible: "Reversible from the backup folder",
  backupConnect: "Back up & Connect",
  linkingHarnesses: "Linking harnesses…",
  rationales: {
    sharedRoot: "Shared user root that Codex and Grok inject into every session",
    vendorBuiltins:
      "Vendor built-ins ship with the app; archived in the vault so ~/.gemini/config/skills stays the only user source",
    codexVendor: "Curated vendor checkout Codex indexes beside ~/.codex/skills",
    bundledMarket: "Bundled marketplace cache of plugin skills",
    staleMarket: "Stale marketplace cache of plugin skills",
    claudePlugins: "Plugin marketplace skills load beside ~/.claude/skills",
    hermesOptional:
      "Optional skill repository Hermes auto-loads beside ~/.hermes/skills",
    grokBundled: "Grok resolves its built-ins here and ignores ~/.grok/skills",
    grokPlugins: "Third-party plugin bundles Grok loads on startup",
  },
  notes: {
    codexSystem: "Internal .system folders are moved into the projection first",
    freebuff: "Manicode shares the Freebuff projection",
    hermesCurator: "Curator state files are kept inside the projection",
  },
  readFailed: (detail: string) => `Could not read the harness bridge: ${detail}`,
  partial: (errors: string) => `Harness bridge partially deployed: ${errors}`,
  backupDetail: (name: string) => `backup ${name}`,
  adopted: (count: number) =>
    `${count} harness-only skill${count === 1 ? "" : "s"} adopted`,
  imported: (sources: string) => `AGENTS.md imported from ${sources}`,
  connected: (bridged: number, total: number, extra: string) =>
    `Harness bridge connected — ${bridged}/${total} harnesses bridged${extra}`,
  deployFailed: (detail: string) =>
    `Could not deploy the harness bridge: ${detail}`,
};
