/// OAuth strategy module: encapsulates the bypass-vs-AuthKit branch
/// behind a single interface. Route handlers consume `oauthStrategy`
/// without knowing which implementation is active. Selection happens
/// once at module load via `config.web.auth_mode`; the production build
/// resolves to `authkit`, vitest / vite dev / docker-compose default to
/// `bypass`.

import { config } from "@tokenoverflow/config";

import type { RequestLogger } from "../logging/request_logger.server";

import { exchangeCode } from "./authkit.server";
import { buildAuthorizeUrl, buildLocalCallbackRedirect } from "./redirects.server";

/// Result of resolving an OAuth `code` into a WorkOS-issued JWT.
/// Strategies hide the difference between the AuthKit token-exchange
/// call and the in-process local stub.
export type ResolveJwtResult = { ok: true; jwt: string } | { ok: false };

export interface ResolveJwtArgs {
  code: string;
  /// Per-request logger threaded down from the route handler so the
  /// `oauth_token_exchange_failed` line carries `request_id` and
  /// `route`. The bypass strategy ignores this; only `authkit` emits.
  log: RequestLogger;
}

export interface OauthStrategy {
  /// Build the URL the BFF redirects to after `/auth/start`. The
  /// `bypass` strategy points at our own `/auth/callback` with a
  /// synthetic code; the `authkit` strategy points at AuthKit's
  /// authorize endpoint.
  buildStartRedirect: (state: string) => string;

  /// Turn an OAuth `code` into a WorkOS-issued JWT.
  resolveJwt: (args: ResolveJwtArgs) => Promise<ResolveJwtResult>;
}

const authkit: OauthStrategy = {
  buildStartRedirect: buildAuthorizeUrl,
  resolveJwt: async ({ code, log }) => {
    const result = await exchangeCode({ code, log });
    return result.ok ? { ok: true, jwt: result.access_token } : { ok: false };
  },
};

const bypass: OauthStrategy = {
  buildStartRedirect: buildLocalCallbackRedirect,
  resolveJwt: async () => {
    // Dynamic import keeps the test private key path out of the prod
    // dependency graph; the module is only resolved when this branch
    // executes. The bypass strategy has no error path beyond a thrown
    // exception, which the route handler will surface, so it ignores
    // the logger argument.
    const { signLocalAuthJwt } = await import("./local_stub.server");
    return { ok: true, jwt: await signLocalAuthJwt() };
  },
};

export const oauthStrategy: OauthStrategy = config.web.auth_mode === "bypass" ? bypass : authkit;
