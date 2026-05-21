import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

beforeEach(() => {
  vi.stubEnv("TOKENOVERFLOW_ENV", "production");
  vi.resetModules();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

async function importModule() {
  return await import("../../../src/utils/auth/redirects.server");
}

describe("buildAuthorizeUrl", () => {
  it("encodes the AuthKit authorize URL with the configured client", async () => {
    const { buildAuthorizeUrl } = await importModule();
    const url = new URL(buildAuthorizeUrl("dead-beef-state"));
    expect(url.origin).toBe("https://intimate-figure-17.authkit.app");
    expect(url.pathname).toBe("/oauth2/authorize");
    expect(url.searchParams.get("client_id")).toBe("client_01KQZW2FG777B71ZK5WG9EKPTW");
    expect(url.searchParams.get("redirect_uri")).toBe("https://app.tokenoverflow.io/auth/callback");
    expect(url.searchParams.get("response_type")).toBe("code");
    expect(url.searchParams.get("scope")).toBe("openid profile email");
    expect(url.searchParams.get("state")).toBe("dead-beef-state");
  });
});

describe("buildLocalCallbackRedirect", () => {
  it("targets /auth/callback with the synthetic code in local mode", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "local");
    vi.resetModules();
    const { buildLocalCallbackRedirect } = await importModule();
    const url = new URL(buildLocalCallbackRedirect("local-state"));
    expect(url.origin).toBe("http://localhost:3000");
    expect(url.pathname).toBe("/auth/callback");
    expect(url.searchParams.get("code")).toBe("local-stub");
    expect(url.searchParams.get("state")).toBe("local-state");
  });
});

describe("buildSuccessRedirect", () => {
  it("returns 302 with the success query and a cleared cookie", async () => {
    const { buildSuccessRedirect } = await importModule();
    const response = buildSuccessRedirect();
    expect(response.status).toBe(302);
    expect(response.headers.get("location")).toBe("https://tokenoverflow.io/?waitlist=success");
    const set_cookie = response.headers.get("set-cookie");
    expect(set_cookie).toBeTruthy();
    expect(set_cookie).toContain("Max-Age=0");
  });
});

describe("buildFailureRedirect", () => {
  it("encodes the reason and clears the cookie", async () => {
    const { buildFailureRedirect } = await importModule();
    const response = buildFailureRedirect("oauth_denied");
    expect(response.status).toBe(302);
    expect(response.headers.get("location")).toBe(
      "https://tokenoverflow.io/?waitlist=error&reason=oauth_denied",
    );
    const set_cookie = response.headers.get("set-cookie");
    expect(set_cookie).toContain("Max-Age=0");
  });

  it.each(["oauth_failed", "state_invalid", "server_error"] as const)(
    "supports the %s reason",
    async (reason) => {
      const { buildFailureRedirect } = await importModule();
      const response = buildFailureRedirect(reason);
      expect(response.headers.get("location")).toBe(
        `https://tokenoverflow.io/?waitlist=error&reason=${reason}`,
      );
    },
  );
});
