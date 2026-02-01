import type { AppConfig } from "../schema.js";

export const productionConfig = {
  env: "production",
  landing: {
    base_url: "https://tokenoverflow.io",
  },
} as const satisfies AppConfig;
