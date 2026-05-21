import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

beforeEach(() => {
  // Anchor to a deterministic env baseline; each describe block flips
  // `TOKENOVERFLOW_WEB_AUTH_MODE` per its scenario.
  vi.stubEnv("TOKENOVERFLOW_ENV", "local");
  vi.stubEnv("TOKENOVERFLOW_WEB_COOKIE_SIGNING_KEY", "unit-key");
  vi.stubEnv("TOKENOVERFLOW_WEB_AUTHKIT_CLIENT_SECRET", "unit-client-secret");
  vi.resetModules();
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  vi.resetModules();
});

describe("oauthStrategy with auth_mode=bypass", () => {
  beforeEach(() => {
    vi.stubEnv("TOKENOVERFLOW_WEB_AUTH_MODE", "bypass");
    vi.resetModules();
  });

  it("buildStartRedirect targets the BFF's own /auth/callback with a synthetic code", async () => {
    const { oauthStrategy } = await import("../../../src/utils/auth/strategy.server");
    const url = new URL(oauthStrategy.buildStartRedirect("local-state"));
    expect(url.origin).toBe("http://localhost:3000");
    expect(url.pathname).toBe("/auth/callback");
    expect(url.searchParams.get("code")).toBe("local-stub");
    expect(url.searchParams.get("state")).toBe("local-state");
  });

  it("resolveJwt returns a locally-signed JWT without calling AuthKit", async () => {
    // Mock the dynamic import so the test does not need a private key
    // file on disk: the bypass strategy resolves `local_auth_stub.server`
    // dynamically, and we replace its `signLocalAuthJwt` export.
    const stubJwt = "stub.local.jwt";
    vi.doMock("../../../src/utils/auth/local_stub.server", () => ({
      signLocalAuthJwt: vi.fn().mockResolvedValue(stubJwt),
    }));

    const { oauthStrategy } = await import("../../../src/utils/auth/strategy.server");
    const { createRequestLogger } = await import("../../../src/utils/logging/logger.server");
    const log = createRequestLogger({ route: "GET /auth/callback" });
    const result = await oauthStrategy.resolveJwt({ code: "ignored-code", log });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.jwt).toBe(stubJwt);
    }
  });
});

describe("oauthStrategy with auth_mode=authkit", () => {
  beforeEach(() => {
    vi.stubEnv("TOKENOVERFLOW_WEB_AUTH_MODE", "authkit");
    vi.resetModules();
  });

  it("buildStartRedirect targets the AuthKit /oauth2/authorize endpoint", async () => {
    const { oauthStrategy } = await import("../../../src/utils/auth/strategy.server");
    const url = new URL(oauthStrategy.buildStartRedirect("real-state"));
    expect(url.origin).toBe("https://intimate-figure-17.authkit.app");
    expect(url.pathname).toBe("/oauth2/authorize");
    expect(url.searchParams.get("client_id")).toBe("client_01KQZW2FG777B71ZK5WG9EKPTW");
    expect(url.searchParams.get("response_type")).toBe("code");
    expect(url.searchParams.get("scope")).toBe("openid profile email");
    expect(url.searchParams.get("state")).toBe("real-state");
  });

  it("resolveJwt forwards the code and logger to exchangeCode and returns the access_token on success", async () => {
    const exchangeCodeMock = vi.fn().mockResolvedValue({
      ok: true,
      access_token: "real-jwt",
      expires_in: 300,
    });
    vi.doMock("../../../src/utils/auth/authkit.server", () => ({
      exchangeCode: exchangeCodeMock,
    }));

    const { oauthStrategy } = await import("../../../src/utils/auth/strategy.server");
    const { createRequestLogger } = await import("../../../src/utils/logging/logger.server");
    const log = createRequestLogger({ route: "GET /auth/callback" });
    const result = await oauthStrategy.resolveJwt({ code: "auth-code", log });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.jwt).toBe("real-jwt");
    }
    expect(exchangeCodeMock).toHaveBeenCalledTimes(1);
    const [arg] = exchangeCodeMock.mock.calls[0]!;
    expect(arg.code).toBe("auth-code");
    // The per-request logger is threaded through so the authkit
    // module emits `oauth_token_exchange_failed` with `request_id`
    // and `route` from the calling context.
    expect(arg.log).toBe(log);
    // The strategy now reads the client secret from config inside the
    // authkit module; it no longer threads it through the call.
    expect(arg).not.toHaveProperty("client_secret");
  });

  it("resolveJwt returns ok=false when exchangeCode reports a failure", async () => {
    vi.doMock("../../../src/utils/auth/authkit.server", () => ({
      exchangeCode: vi.fn().mockResolvedValue({ ok: false }),
    }));

    const { oauthStrategy } = await import("../../../src/utils/auth/strategy.server");
    const { createRequestLogger } = await import("../../../src/utils/logging/logger.server");
    const log = createRequestLogger({ route: "GET /auth/callback" });
    const result = await oauthStrategy.resolveJwt({ code: "auth-code", log });
    expect(result.ok).toBe(false);
  });
});
