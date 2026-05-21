import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  AUTHKIT_TOKEN_URL,
  getCallbackHandler,
  makeCookie,
  mockApiSuccess,
  stubProductionEnv,
  urlOf,
} from "../common/callback_route";
import { captureStdout, countEvents, findEvent } from "../common/capture_stdout";
import { seedRequestId } from "../common/with_request";

beforeEach(() => {
  stubProductionEnv();
  vi.resetModules();
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/// Build a fetch stub returning `token` on AuthKit's token URL and
/// (when supplied) `api` on the `/v1/waitlist` path.
function makeFetchStub(token: Response, api?: Response): ReturnType<typeof vi.fn> {
  return vi.fn(async (input: string | URL | Request) => {
    const url = urlOf(input as string | URL | Request);
    if (url === AUTHKIT_TOKEN_URL) {
      return token;
    }
    if (api !== undefined && url.includes("/v1/waitlist")) {
      return api;
    }
    throw new Error(`unexpected fetch: ${url}`);
  });
}

describe("GET /auth/callback (production)", () => {
  it("happy path: 302 to ?waitlist=success and clears the cookie", async () => {
    const handler = await getCallbackHandler();
    const fetchImpl = mockApiSuccess();
    vi.stubGlobal("fetch", fetchImpl);

    const { lines, restore } = captureStdout();
    try {
      const { cookie, cookieName, payload } = await makeCookie();
      const request = new Request(
        `https://app.tokenoverflow.io/auth/callback?code=auth-code&state=${payload.state}`,
        { headers: { cookie: `${cookieName}=${cookie}`, "x-amzn-requestid": "rid-happy" } },
      );
      await seedRequestId(request);
      const response = await handler({ request });
      expect(response.status).toBe(302);
      expect(response.headers.get("location")).toBe("https://tokenoverflow.io/?waitlist=success");
      expect(response.headers.get("set-cookie")).toContain("Max-Age=0");

      const {
        mock: { calls },
      } = fetchImpl as unknown as { mock: { calls: unknown[][] } };
      const tokenCall = calls.find(
        (c) => urlOf(c[0] as string | URL | Request) === AUTHKIT_TOKEN_URL,
      );
      const tokenInit = tokenCall![1] as RequestInit;
      expect(tokenInit.method).toBe("POST");
      expect((tokenInit.headers as Record<string, string>)["content-type"]).toBe(
        "application/x-www-form-urlencoded",
      );
      expect(Object.fromEntries(new URLSearchParams(tokenInit.body as URLSearchParams))).toEqual({
        grant_type: "authorization_code",
        code: "auth-code",
        client_id: "client_01KQZW2FG777B71ZK5WG9EKPTW",
        client_secret: "integration-client-secret",
        redirect_uri: "https://app.tokenoverflow.io/auth/callback",
      });
      const apiCall = calls.find((c) =>
        urlOf(c[0] as string | URL | Request).includes("/v1/waitlist"),
      );
      expect(((apiCall as unknown[])[0] as Request).headers.get("authorization")).toBe(
        "Bearer stub-jwt",
      );

      const success = findEvent(lines, "oauth_success");
      expect(success).toMatchObject({
        outcome: "success",
        request_id: "rid-happy",
        route: "GET /auth/callback",
        github_id: 99001,
        github_username: "octocat",
        already_on_waitlist: false,
      });
      expect(typeof success!["latency_ms"]).toBe("number");
      expect(countEvents(lines, "oauth_success")).toBe(1);

      // No log line should leak the JWT, OAuth code, cookie, or secret.
      for (const line of lines) {
        expect(line.text).not.toContain("stub-jwt");
        expect(line.text).not.toContain("auth-code");
        expect(line.text).not.toContain(cookie);
        expect(line.text).not.toContain("integration-client-secret");
      }
    } finally {
      restore();
    }
  });

  it("oauth denial -> reason=oauth_denied", async () => {
    const handler = await getCallbackHandler();
    const { lines, restore } = captureStdout();
    try {
      const { cookie, cookieName, payload } = await makeCookie();
      const request = new Request(
        `https://app.tokenoverflow.io/auth/callback?error=access_denied&state=${payload.state}`,
        { headers: { cookie: `${cookieName}=${cookie}`, "x-amzn-requestid": "rid-denied" } },
      );
      await seedRequestId(request);
      const response = await handler({ request });
      expect(response.headers.get("location")).toBe(
        "https://tokenoverflow.io/?waitlist=error&reason=oauth_denied",
      );

      expect(findEvent(lines, "oauth_callback_completed")).toMatchObject({
        outcome: "failure",
        reason: "oauth_denied",
        failure_kind: "provider_error",
        provider_error: "access_denied",
        request_id: "rid-denied",
      });
    } finally {
      restore();
    }
  });

  it("other oauth error -> reason=oauth_failed", async () => {
    const handler = await getCallbackHandler();
    const { lines, restore } = captureStdout();
    try {
      const { cookie, cookieName, payload } = await makeCookie();
      const request = new Request(
        `https://app.tokenoverflow.io/auth/callback?error=server_error&state=${payload.state}`,
        { headers: { cookie: `${cookieName}=${cookie}` } },
      );
      await seedRequestId(request);
      const response = await handler({ request });
      expect(response.headers.get("location")).toBe(
        "https://tokenoverflow.io/?waitlist=error&reason=oauth_failed",
      );
      const terminal = findEvent(lines, "oauth_callback_completed");
      expect(terminal!["reason"]).toBe("oauth_failed");
      expect(terminal!["provider_error"]).toBe("server_error");
    } finally {
      restore();
    }
  });

  it("missing cookie -> reason=state_invalid (falls back to landing base)", async () => {
    const handler = await getCallbackHandler();
    const { lines, restore } = captureStdout();
    try {
      const request = new Request(
        "https://app.tokenoverflow.io/auth/callback?code=abc&state=anything",
      );
      await seedRequestId(request);
      const response = await handler({ request });
      expect(response.headers.get("location")).toBe(
        "https://tokenoverflow.io/?waitlist=error&reason=state_invalid",
      );

      expect(findEvent(lines, "oauth_callback_received")).toMatchObject({ cookie_present: false });
      expect(findEvent(lines, "oauth_callback_completed")).toMatchObject({
        reason: "state_invalid",
        failure_kind: "cookie_missing",
        cookie_present: false,
      });
    } finally {
      restore();
    }
  });

  it("state mismatch -> reason=state_invalid", async () => {
    const handler = await getCallbackHandler();
    const { lines, restore } = captureStdout();
    try {
      const { cookie, cookieName } = await makeCookie({ state: "expected-state" });
      const request = new Request(
        "https://app.tokenoverflow.io/auth/callback?code=abc&state=different-state",
        { headers: { cookie: `${cookieName}=${cookie}` } },
      );
      await seedRequestId(request);
      const response = await handler({ request });
      expect(response.headers.get("location")).toContain("?waitlist=error&reason=state_invalid");

      expect(findEvent(lines, "oauth_state_cookie_invalid")).toMatchObject({
        reason: "state_invalid",
        state_cookie_verify_reason: "state_mismatch",
      });
    } finally {
      restore();
    }
  });

  it("authkit token endpoint error -> reason=oauth_failed", async () => {
    const errBody = JSON.stringify({
      error: "invalid_grant",
      error_description: "authentication code is invalid",
    });
    vi.stubGlobal("fetch", makeFetchStub(new Response(errBody, { status: 400 })));
    const handler = await getCallbackHandler();
    const { lines, restore } = captureStdout();
    try {
      const { cookie, cookieName, payload } = await makeCookie();
      const request = new Request(
        `https://app.tokenoverflow.io/auth/callback?code=auth-code&state=${payload.state}`,
        { headers: { cookie: `${cookieName}=${cookie}` } },
      );
      await seedRequestId(request);
      const response = await handler({ request });
      expect(response.headers.get("location")).toBe(
        "https://tokenoverflow.io/?waitlist=error&reason=oauth_failed",
      );

      expect(findEvent(lines, "oauth_token_exchange_failed")).toMatchObject({
        error_name: "non_2xx",
        error_status: 400,
        error_code: "invalid_grant",
        error_message: "authentication code is invalid",
      });
      expect(findEvent(lines, "oauth_callback_completed")).toMatchObject({
        reason: "oauth_failed",
        failure_kind: "token_exchange_failed",
      });

      // The OAuth code and the client_secret must never reach the logs.
      for (const line of lines) {
        expect(line.text).not.toContain("auth-code");
        expect(line.text).not.toContain("integration-client-secret");
      }
    } finally {
      restore();
    }
  });

  /// Drive an API-failure case end-to-end and assert the two log-line
  /// shape: a non-terminal `oauth_api_call_failed` plus a single
  /// terminal `oauth_callback_completed`.
  async function assertApiFailureBranch(o: {
    status: number;
    redirect_reason: "oauth_failed" | "server_error";
    api_reason: "auth_rejected" | "server_error";
  }): Promise<void> {
    const tokenResp = new Response(JSON.stringify({ access_token: "stub-jwt" }), { status: 200 });
    vi.stubGlobal("fetch", makeFetchStub(tokenResp, new Response("{}", { status: o.status })));
    const handler = await getCallbackHandler();
    const { lines, restore } = captureStdout();
    try {
      const { cookie, cookieName, payload } = await makeCookie();
      const request = new Request(
        `https://app.tokenoverflow.io/auth/callback?code=auth-code&state=${payload.state}`,
        { headers: { cookie: `${cookieName}=${cookie}` } },
      );
      await seedRequestId(request);
      const response = await handler({ request });
      expect(response.headers.get("location")).toBe(
        `https://tokenoverflow.io/?waitlist=error&reason=${o.redirect_reason}`,
      );
      const sibling = findEvent(lines, "oauth_api_call_failed");
      expect(sibling).toMatchObject({ api_status: o.status, api_failure_reason: o.api_reason });
      expect(Object.hasOwn(sibling!, "outcome")).toBe(false);
      expect(Object.hasOwn(sibling!, "latency_ms")).toBe(false);
      const terminal = findEvent(lines, "oauth_callback_completed");
      expect(terminal).toMatchObject({
        outcome: "failure",
        reason: o.redirect_reason,
        failure_kind: "api_call_failed",
        api_status: o.status,
        api_failure_reason: o.api_reason,
      });
      expect(typeof terminal!["latency_ms"]).toBe("number");
      expect(countEvents(lines, "oauth_api_call_failed")).toBe(1);
      expect(countEvents(lines, "oauth_callback_completed")).toBe(1);
    } finally {
      restore();
    }
  }

  it("api 401 -> reason=oauth_failed", async () =>
    await assertApiFailureBranch({
      status: 401,
      redirect_reason: "oauth_failed",
      api_reason: "auth_rejected",
    }));

  it("api 5xx -> reason=server_error", async () =>
    await assertApiFailureBranch({
      status: 503,
      redirect_reason: "server_error",
      api_reason: "server_error",
    }));
});
