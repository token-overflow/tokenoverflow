import { parse } from "valibot";
import { localConfig } from "./environments/local.js";
import { productionConfig } from "./environments/production.js";
import { type AppConfig, AppConfigSchema } from "./schema.js";

const ENVIRONMENTS = {
  local: localConfig,
  production: productionConfig,
} as const satisfies Record<AppConfig["env"], AppConfig>;

type EnvName = keyof typeof ENVIRONMENTS;

const selectEnvironment = (raw: string | undefined): AppConfig => {
  const name = raw === undefined || raw === "" ? "local" : raw;
  if (!(name in ENVIRONMENTS)) {
    throw new Error(`Invalid TOKENOVERFLOW_ENV=${raw}. Expected one of: local, production.`);
  }
  return ENVIRONMENTS[name as EnvName];
};

const loadConfig = (): Readonly<AppConfig> => {
  const selected = selectEnvironment(process.env["TOKENOVERFLOW_ENV"]);
  const parsed = parse(AppConfigSchema, selected);
  return Object.freeze(parsed);
};

export const config: Readonly<AppConfig> = loadConfig();
export { type AppConfig, AppConfigSchema } from "./schema.js";
