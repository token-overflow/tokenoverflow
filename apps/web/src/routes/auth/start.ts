import { config } from "@tokenoverflow/config";
import { createFileRoute } from "@tanstack/solid-router";

import {
  buildStateCookieHeader,
  generateState,
  type OauthStatePayload,
  signStateCookie,
} from "../../utils/auth/cookies.server";
import { oauthStrategy } from "../../utils/auth/strategy.server";
import { startRequestLogger } from "../../utils/logging/route.server";

const ROUTE = "GET /auth/start";

/// `GET /auth/start`. Mints a fresh state token, signs it into a
/// JWT cookie, and redirects per the active OAuth strategy. The strategy
/// abstraction keeps this handler oblivious to whether AuthKit is being
/// hit (`config.web.auth_mode === "authkit"`) or short-circuited via the
/// in-process bypass (`config.web.auth_mode === "bypass"`).
export const Route = createFileRoute("/auth/start")({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const log = startRequestLogger({ route: ROUTE });
        log.info("oauth_start_received", { auth_mode: config.web.auth_mode });

        const url = new URL(request.url);
        const intent = url.searchParams.get("intent");
        if (intent !== "waitlist") {
          log.terminal("oauth_start_completed", {
            outcome: "failure",
            reason: "state_invalid",
            failure_kind: "unknown_intent",
            intent: intent ?? null,
          });
          return new Response("Bad Request: unknown intent", { status: 400 });
        }

        const payload: OauthStatePayload = {
          state: generateState(),
          intent: "waitlist",
        };
        const cookie_value = await signStateCookie(payload, config.web.cookie_signing_key);
        log.info("oauth_state_cookie_issued", { intent: payload.intent });
        const redirect_url = oauthStrategy.buildStartRedirect(payload.state);

        log.terminal("oauth_start_completed", {
          outcome: "success",
          intent: payload.intent,
        });

        return new Response(null, {
          status: 302,
          headers: {
            location: redirect_url,
            "set-cookie": buildStateCookieHeader(cookie_value),
          },
        });
      },
    },
  },
});
