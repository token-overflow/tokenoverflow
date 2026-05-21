import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { captureStdout, findEvent } from "../common/capture_stdout";
import { seedRequestId } from "../common/with_request";

async function importCookieName(): Promise<string> {
  // Dynamic import after `vi.stubEnv` so the cookie module's
  // env-gated `COOKIE_NAME` constant resolves correctly per test.
  const { OAUTH_STATE_COOKIE_NAME } = await import("../../src/utils/auth/cookies.server");
  return OAUTH_STATE_COOKIE_NAME;
}

beforeEach(() => {
  vi.stubEnv("TOKENOVERFLOW_ENV", "production");
  vi.stubEnv("TOKENOVERFLOW_WEB_COOKIE_SIGNING_KEY", "integration-key");
  vi.resetModules();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

interface RouteHandlerCtx {
  request: Request;
}

type StartHandler = (ctx: RouteHandlerCtx) => Promise<Response>;

interface MockedRouteOptions {
  options: {
    server: {
      handlers: {
        GET: StartHandler;
      };
    };
  };
}

async function getStartHandler(): Promise<StartHandler> {
  // The vitest setup file (`vitest.setup.ts`) stubs `createFileRoute` so
  // the route module evaluates its options without booting the SSR
  // runtime. The recorded options expose the underlying handler.
  const mod = (await import("../../src/routes/auth/start")) as unknown as {
    Route: MockedRouteOptions;
  };
  return mod.Route.options.server.handlers.GET;
}

describe("GET /auth/start (production)", () => {
  it("redirects to AuthKit with state, scope, and a Set-Cookie", async () => {
    const handler = await getStartHandler();
    const cookieName = await importCookieName();
    const { lines, restore } = captureStdout();
    try {
      const request = new Request("https://app.tokenoverflow.io/auth/start?intent=waitlist", {
        headers: { "x-amzn-requestid": "rid-start-1" },
      });
      await seedRequestId(request);
      const response = await handler({ request });

      expect(response.status).toBe(302);
      const location = response.headers.get("location");
      expect(location).toBeTruthy();
      const url = new URL(location!);
      expect(url.origin).toBe("https://intimate-figure-17.authkit.app");
      expect(url.pathname).toBe("/oauth2/authorize");
      expect(url.searchParams.get("scope")).toBe("openid profile email");
      expect(url.searchParams.get("client_id")).toBe("client_01KQZW2FG777B71ZK5WG9EKPTW");
      expect(url.searchParams.get("state")).toBeTruthy();

      const cookie = response.headers.get("set-cookie");
      expect(cookie).toBeTruthy();
      // Production cookie name uses the `__Host-` prefix; one literal
      // assertion documents the prod wire format. Other assertions go via
      // `cookieName` so a future rename is one-line.
      expect(cookieName).toBe("__Host-oauth_state");
      // Compact-serialization JWT: three base64url segments separated by `.`.
      expect(cookie).toMatch(new RegExp(`^${cookieName}=[^;]+\\.[^;]+\\.[^;]+;`));
      expect(cookie).toContain("HttpOnly");
      expect(cookie).toContain("Secure");
      expect(cookie).toContain("SameSite=Lax");
      expect(cookie).toContain("Max-Age=600");
      expect(cookie).toContain("Path=/");

      const received = findEvent(lines, "oauth_start_received");
      expect(received).toBeDefined();
      expect(received!["request_id"]).toBe("rid-start-1");
      expect(received!["route"]).toBe("GET /auth/start");
      expect(received!["auth_mode"]).toBe("authkit");

      const issued = findEvent(lines, "oauth_state_cookie_issued");
      expect(issued).toBeDefined();
      expect(issued!["intent"]).toBe("waitlist");
      expect(issued!["request_id"]).toBe("rid-start-1");

      const completed = findEvent(lines, "oauth_start_completed");
      expect(completed).toBeDefined();
      expect(completed!["outcome"]).toBe("success");
      expect(completed!["intent"]).toBe("waitlist");
      expect(typeof completed!["latency_ms"]).toBe("number");

      // The state value itself must never appear in logs.
      const stateValue = url.searchParams.get("state")!;
      for (const line of lines) {
        expect(line.text).not.toContain(stateValue);
      }
    } finally {
      restore();
    }
  });

  it("rejects an unknown intent with 400", async () => {
    const handler = await getStartHandler();
    const { lines, restore } = captureStdout();
    try {
      const request = new Request("https://app.tokenoverflow.io/auth/start?intent=other");
      await seedRequestId(request);
      const response = await handler({ request });
      expect(response.status).toBe(400);

      const completed = findEvent(lines, "oauth_start_completed");
      expect(completed).toBeDefined();
      expect(completed!["outcome"]).toBe("failure");
      expect(completed!["reason"]).toBe("state_invalid");
      expect(completed!["failure_kind"]).toBe("unknown_intent");
      expect(completed!["intent"]).toBe("other");
    } finally {
      restore();
    }
  });
});

describe("GET /auth/start (auth_mode=bypass)", () => {
  beforeEach(() => {
    // `auth_mode=bypass` is the schema default; setting it explicitly
    // here documents the gate the handler now reads instead of the old
    // `config.env === "local"` branch.
    vi.stubEnv("TOKENOVERFLOW_ENV", "local");
    vi.stubEnv("TOKENOVERFLOW_WEB_AUTH_MODE", "bypass");
    vi.resetModules();
  });

  it("redirects directly to /auth/callback with a synthetic code", async () => {
    const handler = await getStartHandler();
    const request = new Request("http://localhost:3000/auth/start?intent=waitlist");
    const response = await handler({ request });
    expect(response.status).toBe(302);
    const location = response.headers.get("location");
    const url = new URL(location!);
    expect(url.origin).toBe("http://localhost:3000");
    expect(url.pathname).toBe("/auth/callback");
    expect(url.searchParams.get("code")).toBe("local-stub");
    expect(url.searchParams.get("state")).toBeTruthy();

    const cookie = response.headers.get("set-cookie");
    expect(cookie).toBeTruthy();
    // The cookie name and `Secure` attribute do NOT change in local
    // mode: modern browsers honor `Secure` cookies on HTTP-localhost
    // because localhost is a potentially-trustworthy origin per the
    // W3C Secure Contexts spec, so `__Host-` is accepted there too.
    expect(cookie).toContain("__Host-oauth_state=");
    expect(cookie).toContain("Secure");
  });
});
