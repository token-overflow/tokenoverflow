import { createFileRoute } from "@tanstack/solid-router";
import { config } from "@tokenoverflow/config";

import { addToWaitlist } from "../../utils/api/waitlist.server";
import { readStateCookie, verifyStateCookie } from "../../utils/auth/cookies.server";
import { buildFailureRedirect, buildSuccessRedirect } from "../../utils/auth/redirects.server";
import { parseCallbackQuery } from "../../utils/auth/schemas";
import { oauthStrategy } from "../../utils/auth/strategy.server";
import { startRequestLogger } from "../../utils/logging/route.server";

const ROUTE = "GET /auth/callback";

/// `GET /auth/callback`. Validates the state cookie, drives the OAuth
/// round-trip via the active strategy, calls the API, and returns the
/// redirect.
export const Route = createFileRoute("/auth/callback")({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const log = startRequestLogger({ route: ROUTE });
        const url = new URL(request.url);
        const query_object: Record<string, string> = {};
        for (const [key, value] of url.searchParams) {
          query_object[key] = value;
        }
        const parsed = parseCallbackQuery(query_object);
        const cookie_value = readStateCookie(request);
        const cookie_present = cookie_value !== undefined;

        let query_kind: "code" | "error" | "invalid";
        if (!parsed.ok) {
          query_kind = "invalid";
        } else if ("error" in parsed.query) {
          query_kind = "error";
        } else {
          query_kind = "code";
        }
        log.info("oauth_callback_received", {
          auth_mode: config.web.auth_mode,
          cookie_present,
          query_kind,
        });

        if (!cookie_value || !parsed.ok) {
          log.terminal("oauth_callback_completed", {
            outcome: "failure",
            reason: "state_invalid",
            failure_kind: cookie_value ? "query_parse_failed" : "cookie_missing",
            cookie_present,
          });
          return buildFailureRedirect("state_invalid");
        }

        const { query } = parsed;
        const verified = await verifyStateCookie({
          cookie_value,
          expected_state: query.state,
          signing_key: config.web.cookie_signing_key,
        });
        if (!verified.ok) {
          log.terminal("oauth_state_cookie_invalid", {
            outcome: "failure",
            reason: "state_invalid",
            cookie_present,
            state_cookie_verify_reason: verified.reason,
          });
          return buildFailureRedirect("state_invalid");
        }

        if ("error" in query) {
          const reason = query.error === "access_denied" ? "oauth_denied" : "oauth_failed";
          log.terminal("oauth_callback_completed", {
            outcome: "failure",
            reason,
            failure_kind: "provider_error",
            state_cookie_verify_reason: "ok",
            // The OAuth `error` parameter is a fixed identifier (e.g.
            // `access_denied`, `server_error`) per RFC 6749 S4.1.2.1; it
            // is safe to log because it is not a credential.
            provider_error: query.error,
          });
          return buildFailureRedirect(reason);
        }

        const jwt_result = await oauthStrategy.resolveJwt({ code: query.code, log });
        if (!jwt_result.ok) {
          log.terminal("oauth_callback_completed", {
            outcome: "failure",
            reason: "oauth_failed",
            failure_kind: "token_exchange_failed",
            state_cookie_verify_reason: "ok",
          });
          return buildFailureRedirect("oauth_failed");
        }

        const api_result = await addToWaitlist({ workos_jwt: jwt_result.jwt });
        if (!api_result.ok) {
          const reason = api_result.reason === "auth_rejected" ? "oauth_failed" : "server_error";
          log.warn("oauth_api_call_failed", {
            api_status: api_result.status ?? null,
            api_failure_reason: api_result.reason,
          });
          log.terminal("oauth_callback_completed", {
            outcome: "failure",
            reason,
            failure_kind: "api_call_failed",
            state_cookie_verify_reason: "ok",
            api_status: api_result.status ?? null,
            api_failure_reason: api_result.reason,
          });
          return buildFailureRedirect(reason);
        }

        log.terminal("oauth_success", {
          outcome: "success",
          state_cookie_verify_reason: "ok",
          github_id: api_result.data.github_id,
          github_username: api_result.data.github_username,
          already_on_waitlist: api_result.data.already_on_waitlist,
        });
        return buildSuccessRedirect();
      },
    },
  },
});
