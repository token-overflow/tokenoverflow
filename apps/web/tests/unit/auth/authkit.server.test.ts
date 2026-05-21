import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { RequestLogger } from "../../../src/utils/logging/logger.server";

import { captureStdout, findEvent } from "../../common/capture_stdout";

const TOKEN_URL = "https://intimate-figure-17.authkit.app/oauth2/token";
const CLIENT_ID = "client_01KQZW2FG777B71ZK5WG9EKPTW";

beforeEach(() => {
  vi.stubEnv("TOKENOVERFLOW_ENV", "production");
  // Stub the secret env var BEFORE the dynamic import so the shared
  // config picks it up at module load. The shared config caches at
  // load time; `vi.resetModules()` below evicts the cached instance.
  vi.stubEnv("TOKENOVERFLOW_WEB_AUTHKIT_CLIENT_SECRET", "secret");
  vi.resetModules();
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/// Build a per-request `RequestLogger` bound to a known `route`. Tests
/// assert this stamps the emitted log lines so the
/// `oauth_token_exchange_failed` line is correlatable with the
/// terminal `oauth_callback_completed` line. `request_id` is wired
/// through pino-lambda's `withRequest` separately.
async function makeLogger(): Promise<{
  log: RequestLogger;
  request_id: string;
  route: string;
}> {
  const request_id = "rid-exchange-1";
  const route = "GET /auth/callback";
  const { createRequestLogger, withRequest } =
    await import("../../../src/utils/logging/logger.server");
  withRequest(
    { headers: { "x-amzn-requestid": request_id } } as never,
    { awsRequestId: request_id } as never,
  );
  return { log: createRequestLogger({ route }), request_id, route };
}

interface FetchCall {
  url: string;
  init: RequestInit | undefined;
}

function urlOf(input: string | URL | Request): string {
  if (typeof input === "string") {
    return input;
  }
  if (input instanceof URL) {
    return input.toString();
  }
  return input.url;
}

function captureFetch(impl: () => Promise<Response>): {
  fetchMock: ReturnType<typeof vi.fn>;
  calls: FetchCall[];
} {
  const calls: FetchCall[] = [];
  const fetchMock = vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: urlOf(input), init });
    return await impl();
  });
  vi.stubGlobal("fetch", fetchMock);
  return { fetchMock, calls };
}

describe("exchangeCode", () => {
  it("posts the form-encoded body to the AuthKit token URL and returns the access_token on 200", async () => {
    const { calls } = captureFetch(
      async () =>
        new Response(
          JSON.stringify({ access_token: "jwt-value", token_type: "Bearer", expires_in: 300 }),
          { status: 200, headers: { "content-type": "application/json" } },
        ),
    );
    const { exchangeCode } = await import("../../../src/utils/auth/authkit.server");
    const { log } = await makeLogger();

    const result = await exchangeCode({ code: "auth-code", log });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.access_token).toBe("jwt-value");
    }

    expect(calls).toHaveLength(1);
    expect(calls[0]!.url).toBe(TOKEN_URL);
    const init = calls[0]!.init!;
    expect(init.method).toBe("POST");
    const headers = init.headers as Record<string, string>;
    expect(headers["content-type"]).toBe("application/x-www-form-urlencoded");
    expect(headers["accept"]).toBe("application/json");
    const body = new URLSearchParams(init.body as URLSearchParams);
    expect(body.get("grant_type")).toBe("authorization_code");
    expect(body.get("code")).toBe("auth-code");
    expect(body.get("client_id")).toBe(CLIENT_ID);
    expect(body.get("client_secret")).toBe("secret");
    expect(body.get("redirect_uri")).toBe("https://app.tokenoverflow.io/auth/callback");
  });

  it("returns ok=false and emits a structured `non_2xx` log line when AuthKit returns 400 invalid_grant", async () => {
    captureFetch(
      async () =>
        new Response(
          JSON.stringify({
            error: "invalid_grant",
            error_description: "authentication code is invalid",
          }),
          { status: 400, headers: { "content-type": "application/json" } },
        ),
    );
    const { lines, restore } = captureStdout();
    try {
      const { exchangeCode } = await import("../../../src/utils/auth/authkit.server");
      const { log, request_id, route } = await makeLogger();
      const result = await exchangeCode({ code: "bad-code", log });
      expect(result.ok).toBe(false);

      const event = findEvent(lines, "oauth_token_exchange_failed");
      expect(event).toBeDefined();
      expect(event!["error_name"]).toBe("non_2xx");
      expect(event!["error_status"]).toBe(400);
      expect(event!["error_code"]).toBe("invalid_grant");
      expect(event!["error_message"]).toBe("authentication code is invalid");
      // Per-request context must be threaded through so ops can pivot
      // from this line to the terminal `oauth_callback_completed` by
      // request id.
      expect(event!["request_id"]).toBe(request_id);
      expect(event!["route"]).toBe(route);
      // Critical: the OAuth code and client_secret must never appear in
      // any log line.
      for (const line of lines) {
        expect(line.text).not.toContain("bad-code");
        expect(line.text).not.toContain('"secret"');
      }
    } finally {
      restore();
    }
  });

  it("returns ok=false and emits a `fetch_failed` log line when fetch throws", async () => {
    const fetchMock = vi.fn().mockRejectedValue(new Error("network down"));
    vi.stubGlobal("fetch", fetchMock);
    const { lines, restore } = captureStdout();
    try {
      const { exchangeCode } = await import("../../../src/utils/auth/authkit.server");
      const { log } = await makeLogger();
      const result = await exchangeCode({ code: "auth-code", log });
      expect(result.ok).toBe(false);

      const event = findEvent(lines, "oauth_token_exchange_failed");
      expect(event).toBeDefined();
      expect(event!["error_name"]).toBe("fetch_failed");
      expect(event!["error_message"]).toBe("network down");
      // No log line should ever leak the OAuth code or client_secret.
      for (const line of lines) {
        expect(line.text).not.toContain("auth-code");
        expect(line.text).not.toContain('"secret"');
      }
    } finally {
      restore();
    }
  });

  it("returns ok=false and emits a `malformed_response` log line when the 2xx body is not valid JSON", async () => {
    captureFetch(
      async () =>
        new Response("not-json", {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
    );
    const { lines, restore } = captureStdout();
    try {
      const { exchangeCode } = await import("../../../src/utils/auth/authkit.server");
      const { log } = await makeLogger();
      const result = await exchangeCode({ code: "auth-code", log });
      expect(result.ok).toBe(false);

      const event = findEvent(lines, "oauth_token_exchange_failed");
      expect(event).toBeDefined();
      expect(event!["error_name"]).toBe("malformed_response");
      expect(event!["error_status"]).toBe(200);
    } finally {
      restore();
    }
  });

  it("returns ok=false and emits a `malformed_response` log line when the 2xx body lacks `access_token`", async () => {
    captureFetch(
      async () =>
        new Response(JSON.stringify({ token_type: "Bearer" }), {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
    );
    const { lines, restore } = captureStdout();
    try {
      const { exchangeCode } = await import("../../../src/utils/auth/authkit.server");
      const { log } = await makeLogger();
      const result = await exchangeCode({ code: "auth-code", log });
      expect(result.ok).toBe(false);

      const event = findEvent(lines, "oauth_token_exchange_failed");
      expect(event).toBeDefined();
      expect(event!["error_name"]).toBe("malformed_response");
      expect(event!["error_status"]).toBe(200);
    } finally {
      restore();
    }
  });

  it("caps the upstream `error_description` at 200 chars on non-2xx", async () => {
    const huge = "x".repeat(5_000);
    captureFetch(
      async () =>
        new Response(JSON.stringify({ error: "invalid_grant", error_description: huge }), {
          status: 400,
          headers: { "content-type": "application/json" },
        }),
    );
    const { lines, restore } = captureStdout();
    try {
      const { exchangeCode } = await import("../../../src/utils/auth/authkit.server");
      const { log } = await makeLogger();
      const result = await exchangeCode({ code: "bad-code", log });
      expect(result.ok).toBe(false);

      const event = findEvent(lines, "oauth_token_exchange_failed");
      expect(event).toBeDefined();
      const captured = event!["error_message"] as string;
      expect(captured).toHaveLength(200);
      expect(captured).toBe("x".repeat(200));
    } finally {
      restore();
    }
  });
});
