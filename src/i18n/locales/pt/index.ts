import { agents, common, footer, scopes, sidebar, store, tokens } from "./common";
import { skills } from "./skills";
import { mcps } from "./mcps";
import { sessions } from "./sessions";
import { settings } from "./settings";
import { presets } from "./presets";
import { bridge } from "./bridge";
import { ideas } from "./ideas";
import { janitor } from "./janitor";
import { credentials } from "./credentials";

/**
 * Master Portuguese dictionary.
 *
 * Its inferred type is the single source of truth for the whole app: the
 * English index is annotated with `TranslationSchema`, so adding a key here
 * fails the build until the English copy exists too.
 */
export const pt = {
  common,
  store,
  scopes,
  sidebar,
  skills,
  mcps,
  sessions,
  settings,
  agents,
  tokens,
  bridge,
  ideas,
  janitor,
  footer,
  presets,
  credentials,
};

export type TranslationSchema = typeof pt;

export default pt;
