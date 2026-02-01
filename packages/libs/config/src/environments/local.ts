import type { AppConfig } from "../schema.js";

export const localConfig = {
  env: "local",
  landing: {
    base_url: "http://localhost:4321",
  },
} as const satisfies AppConfig;
