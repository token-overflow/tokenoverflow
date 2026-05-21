import convict from "convict";
import convictFormatWithValidator from "convict-format-with-validator";

/// `format: "url"` (and `email`, `ipaddress`) live in the optional
/// `convict-format-with-validator` package.
convict.addFormats(convictFormatWithValidator);

/// Strip empty `TOKENOVERFLOW_*` entries before handing the map to
/// convict. Docker compose's `${VAR:-}` interpolation produces an
/// empty string when the host has no value set; convict treats `""`
/// as a literal, which would defeat the schema defaults (especially
/// the `env` picklist) and force every consumer to set every var.
const filteredEnv: NodeJS.ProcessEnv = {};
for (const [key, value] of Object.entries(process.env)) {
  if (value !== "") {
    filteredEnv[key] = value;
  }
}

/// Deep-partial helper used by per-env override modules so they only
/// need to declare the fields that differ from the schema defaults.
/// Convict's `load()` merges objects recursively, so missing keys
/// keep their defaults.
export type DeepPartial<T> = T extends object ? { [K in keyof T]?: DeepPartial<T[K]> } : T;

export interface AppConfig {
  env: "local" | "production";
  landing: {
    base_url: string;
  };
  api: {
    base_url: string;
  };
  web: {
    base_url: string;
    auth_mode: "bypass" | "authkit";
    authkit: {
      issuer: string;
      authorize_url: string;
      token_url: string;
      client_id: string;
      client_secret: string;
    };
    cookie_signing_key: string;
    workos_id: string;
  };
}

export const configSchema = convict<AppConfig>(
  {
    env: {
      format: ["local", "production"],
      default: "local",
      env: "TOKENOVERFLOW_ENV",
      doc: "Selects which per-env override file is layered on top of the schema defaults.",
    },
    landing: {
      base_url: {
        format: "url",
        default: "http://localhost:4321",
        doc: "Public URL of the static landing site; the BFF redirects here after the OAuth flow.",
      },
    },
    api: {
      base_url: {
        format: "url",
        default: "http://localhost:8080",
        env: "TOKENOVERFLOW_WEB_API_BASE_URL",
        doc: "Rust API base URL the BFF calls. Docker compose overrides via env so the BFF reaches the API by service name.",
      },
    },
    web: {
      base_url: {
        format: "url",
        default: "http://localhost:3000",
        doc: "Public URL of the BFF; the landing CTA targets this host.",
      },
      auth_mode: {
        format: ["bypass", "authkit"],
        default: "bypass",
        env: "TOKENOVERFLOW_WEB_AUTH_MODE",
        doc: "Selects the OAuth strategy. 'bypass' short-circuits AuthKit and signs JWTs locally with the test key (used in docker-compose, vitest, vite dev). 'authkit' calls the real AuthKit token endpoint. Production overrides to 'authkit' via the per-env file.",
      },
      authkit: {
        issuer: {
          format: "url",
          default: "https://intimate-figure-17.authkit.app",
          doc: "AuthKit issuer URL (also the JWT iss claim).",
        },
        authorize_url: {
          format: "url",
          default: "https://intimate-figure-17.authkit.app/oauth2/authorize",
          doc: "AuthKit authorize endpoint the BFF redirects to.",
        },
        token_url: {
          format: "url",
          default: "https://intimate-figure-17.authkit.app/oauth2/token",
          doc: "AuthKit token endpoint the BFF posts the OAuth code to.",
        },
        client_id: {
          format: String,
          default: "client_01KQZW2FG777B71ZK5WG9EKPTW",
          doc: "WorkOS confidential client ID for the `TokenOverflow Web` Connect App; same across environments today.",
        },
        client_secret: {
          format: String,
          default: "",
          env: "TOKENOVERFLOW_WEB_AUTHKIT_CLIENT_SECRET",
          sensitive: true,
          doc: "WorkOS confidential client secret for the `TokenOverflow Web` Connect App. Used by the BFF's raw-fetch token exchange against the AuthKit token endpoint. Empty in local because AuthKit calls are bypassed.",
        },
      },
      cookie_signing_key: {
        format: String,
        default: "localdev",
        env: "TOKENOVERFLOW_WEB_COOKIE_SIGNING_KEY",
        sensitive: true,
        doc: "HMAC key for the OAuth state cookie. Production reads from SSM.",
      },
      workos_id: {
        format: String,
        default: "test-voter",
        env: "TOKENOVERFLOW_WEB_WORKOS_ID",
        doc: "WorkOS ID the local AuthKit bypass mints test JWTs for. Empty / unused in production.",
      },
    },
  },
  { env: filteredEnv },
);
