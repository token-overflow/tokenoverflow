import { localOverrides } from "./environments/local.js";
import { productionOverrides } from "./environments/production.js";
import { type AppConfig, configSchema } from "./schema.js";

const env = configSchema.get("env");

if (env === "production") {
  configSchema.load(productionOverrides);
} else if (env === "local") {
  configSchema.load(localOverrides);
}

configSchema.validate({ allowed: "strict" });

export const config: Readonly<AppConfig> = Object.freeze(configSchema.getProperties());
export type { AppConfig } from "./schema.js";
