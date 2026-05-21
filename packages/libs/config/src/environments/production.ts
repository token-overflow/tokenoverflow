import type { AppConfig, DeepPartial } from "../schema.js";

export const productionOverrides = {
  landing: { base_url: "https://tokenoverflow.io" },
  api: { base_url: "https://pmbh5f29x3.execute-api.us-east-1.amazonaws.com" },
  web: {
    base_url: "https://app.tokenoverflow.io",
    auth_mode: "authkit",
    cookie_signing_key: "",
    workos_id: "",
  },
} satisfies DeepPartial<AppConfig>;
