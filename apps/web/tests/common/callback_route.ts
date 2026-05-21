/// Shared helpers for `tests/integration/routes_callback*.test.ts`.
///
/// Centralises the repeated import-handler / sign-cookie / mock-fetch
/// shape so each test file stays focused on the case it covers. The
/// rewritten BFF talks to AuthKit with `fetch()` instead of the WorkOS
/// SDK, so the helpers expose a fetch mock that handles both the
/// AuthKit token URL and the Rust API endpoint.

import { vi } from "vitest";

import type { OauthStatePayload } from "../../src/utils/auth/cookies.server";

export const SIGNING_KEY = "integration-key";
export const AUTHKIT_TOKEN_URL = "https://intimate-figure-17.authkit.app/oauth2/token";

export interface RouteHandlerCtx {
  request: Request;
}

export type CallbackHandler = (ctx: RouteHandlerCtx) => Promise<Response>;

interface MockedRouteOptions {
  options: {
    server: {
      handlers: {
        GET: CallbackHandler;
      };
    };
  };
}

/// Pull the callback handler out of the recorded route options. The
/// vitest setup file (`vitest.setup.ts`) stubs `createFileRoute` so the
/// route module evaluates without booting the SSR runtime.
export async function getCallbackHandler(): Promise<CallbackHandler> {
  const mod = (await import("../../src/routes/auth/callback")) as unknown as {
    Route: MockedRouteOptions;
  };
  return mod.Route.options.server.handlers.GET;
}

export interface MakeCookieOptions {
  state?: string;
}

/// Sign a state cookie with the integration signing key and return the
/// pieces a test needs to forge the inbound request. Imports the cookie
/// module dynamically so the env-stub takes effect first.
export async function makeCookie(opts: MakeCookieOptions = {}): Promise<{
  cookie: string;
  payload: OauthStatePayload;
  cookieName: string;
}> {
  const { OAUTH_STATE_COOKIE_NAME, signStateCookie } =
    await import("../../src/utils/auth/cookies.server");
  const payload: OauthStatePayload = {
    state: opts.state ?? "real-state-deadbeef",
    intent: "waitlist",
  };
  const cookie = await signStateCookie(payload, SIGNING_KEY);
  return { cookie, payload, cookieName: OAUTH_STATE_COOKIE_NAME };
}

/// hey-api passes a `Request` instance to fetch. The `Request` object's
/// `.toString()` returns `[object Request]`, so we read `.url`
/// explicitly when the input isn't a plain string.
export function urlOf(input: string | URL | Request): string {
  if (typeof input === "string") {
    return input;
  }
  if (input instanceof URL) {
    return input.toString();
  }
  return input.url;
}

/// Replace `globalThis.fetch` with a stub that handles both the AuthKit
/// token URL (returns a stub access_token) and the Rust API's
/// `/v1/waitlist` (returns 201). Returns the underlying `vi.fn` so the
/// test can assert on calls (URL, method, headers, body).
export function mockApiSuccess(): typeof fetch {
  return vi.fn().mockImplementation(async (input: string | URL | Request) => {
    const url = urlOf(input);
    if (url === AUTHKIT_TOKEN_URL) {
      return new Response(JSON.stringify({ access_token: "stub-jwt" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }
    if (url.includes("/v1/waitlist")) {
      return new Response(
        JSON.stringify({
          github_id: 99001,
          github_username: "octocat",
          already_on_waitlist: false,
        }),
        { status: 201, headers: { "content-type": "application/json" } },
      );
    }
    throw new Error(`unexpected fetch: ${url}`);
  }) as unknown as typeof fetch;
}

/// Apply the env stubs the production-mode integration test relies on
/// before importing the route handler.
export function stubProductionEnv(): void {
  vi.stubEnv("TOKENOVERFLOW_ENV", "production");
  vi.stubEnv("TOKENOVERFLOW_WEB_COOKIE_SIGNING_KEY", SIGNING_KEY);
  vi.stubEnv("TOKENOVERFLOW_WEB_AUTHKIT_CLIENT_SECRET", "integration-client-secret");
}
