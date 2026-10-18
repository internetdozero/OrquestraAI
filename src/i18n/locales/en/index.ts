import type { TranslationSchema } from "../pt";
import { agents, common, footer, scopes, sidebar, store, tokens } from "./common";
import { skills } from "./skills";
import { mcps } from "./mcps";
import { sessions } from "./sessions";
import { settings } from "./settings";
import { presets } from "./presets";
import { bridge } from "./bridge";
import { ideas } from "./ideas";
import { janitor } from "./janitor";

/**
 * English dictionary. Annotating it with `TranslationSchema` makes a missing
 * key, an extra key or a mismatched parameter list a compile error.
 */
const en: TranslationSchema = {
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
};

export default en;
