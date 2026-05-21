import { afterEach, describe, expect, it, vi } from "vitest";

describe("config loader", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.resetModules();
  });

  it("defaults to local when TOKENOVERFLOW_ENV is unset", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(config.env).toBe("local");
    expect(config.landing.base_url).toBe("http://localhost:4321");
    expect(config.api.base_url).toBe("http://localhost:8080");
    expect(config.web.base_url).toBe("http://localhost:3000");
    expect(config.web.auth_mode).toBe("bypass");
    expect(config.web.authkit.client_id).toBe("client_01KQZW2FG777B71ZK5WG9EKPTW");
    expect(config.web.cookie_signing_key).toBe("localdev");
    expect(config.web.authkit.client_secret).toBe("");
    expect(config.web.workos_id).toBe("test-voter");
  });

  it("returns production values when TOKENOVERFLOW_ENV=production", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "production");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(config.env).toBe("production");
    expect(config.landing.base_url).toBe("https://tokenoverflow.io");
    expect(config.api.base_url).toBe("https://pmbh5f29x3.execute-api.us-east-1.amazonaws.com");
    expect(config.web.base_url).toBe("https://app.tokenoverflow.io");
    expect(config.web.auth_mode).toBe("authkit");
    expect(config.web.authkit.issuer).toBe("https://intimate-figure-17.authkit.app");
    expect(config.web.authkit.authorize_url).toBe(
      "https://intimate-figure-17.authkit.app/oauth2/authorize",
    );
    expect(config.web.authkit.token_url).toBe(
      "https://intimate-figure-17.authkit.app/oauth2/token",
    );
    expect(config.web.cookie_signing_key).toBe("");
    expect(config.web.authkit.client_secret).toBe("");
    expect(config.web.workos_id).toBe("");
  });

  it("throws a clear error when TOKENOVERFLOW_ENV is unknown", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "staging");
    vi.resetModules();

    // Convict's `format: ["local", "production"]` validates the env value
    // when the env binding is read; loading the module triggers it.
    await expect(import("../../src/index.js")).rejects.toThrow(/staging/);
  });

  it("returns a frozen config object", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "local");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(Object.isFrozen(config)).toBe(true);
  });

  it("rejects unknown top-level keys under allowed: strict", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "local");
    vi.resetModules();
    const { configSchema } = await import("../../src/schema.js");

    // Smoke-test convict's strict-mode validation: a key that is not in
    // the schema declaration must trip validate({ allowed: "strict" }).
    configSchema.load({ rogue_key: "nope" } as never);
    expect(() => configSchema.validate({ allowed: "strict" })).toThrow();
  });
});

describe("env-var overrides", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.resetModules();
  });

  it("TOKENOVERFLOW_WEB_API_BASE_URL overrides config.api.base_url", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "local");
    vi.stubEnv("TOKENOVERFLOW_WEB_API_BASE_URL", "http://api:8080");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(config.api.base_url).toBe("http://api:8080");
  });

  it("TOKENOVERFLOW_WEB_COOKIE_SIGNING_KEY overrides config.web.cookie_signing_key", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "production");
    vi.stubEnv("TOKENOVERFLOW_WEB_COOKIE_SIGNING_KEY", "explicit-key");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(config.web.cookie_signing_key).toBe("explicit-key");
  });

  it("TOKENOVERFLOW_WEB_AUTHKIT_CLIENT_SECRET overrides config.web.authkit.client_secret", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "production");
    vi.stubEnv("TOKENOVERFLOW_WEB_AUTHKIT_CLIENT_SECRET", "production-client-secret");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(config.web.authkit.client_secret).toBe("production-client-secret");
  });

  it("TOKENOVERFLOW_WEB_WORKOS_ID overrides config.web.workos_id", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "local");
    vi.stubEnv("TOKENOVERFLOW_WEB_WORKOS_ID", "00000000-0000-0000-0000-000000000123");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(config.web.workos_id).toBe("00000000-0000-0000-0000-000000000123");
  });

  it("TOKENOVERFLOW_WEB_AUTH_MODE flips config.web.auth_mode to authkit in local", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "local");
    vi.stubEnv("TOKENOVERFLOW_WEB_AUTH_MODE", "authkit");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(config.web.auth_mode).toBe("authkit");
  });

  it("preserves the per-environment defaults when no overrides are set", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "local");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(config.api.base_url).toBe("http://localhost:8080");
    expect(config.web.cookie_signing_key).toBe("localdev");
    expect(config.web.authkit.client_secret).toBe("");
    expect(config.web.workos_id).toBe("test-voter");
  });
});
