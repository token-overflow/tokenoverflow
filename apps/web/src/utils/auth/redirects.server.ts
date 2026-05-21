/// Redirect URL and `Response` builders for the OAuth round-trip.
///
/// Splits cleanly into two layers:
///   - URL builders (`buildAuthorizeUrl`, `buildLocalCallbackRedirect`)
///     return a string the start handler hands to the user agent.
///   - Response builders (`buildSuccessRedirect`, `buildFailureRedirect`)
///     return a 302 `Response` with the cookie cleared, used by the
///     callback handler.

import { config } from "@tokenoverflow/config";

import { clearCookieHeader } from "./cookies.server";

/// Reason taxonomy surfaced to the landing page via
/// `?waitlist=error&reason=<reason>`. Kept in sync with the landing
/// popup's `data-state` matcher.
export type CallbackReason = "oauth_denied" | "oauth_failed" | "state_invalid" | "server_error";

/// Build the AuthKit `/oauth2/authorize` URL for a confidential client.
///
/// The `state` is the cookie-bound CSRF token. `redirect_uri` is the BFF
/// callback URL. Scope is `openid profile email`; the email lands on the
/// access-token JWT only because the WorkOS Dashboard JWT Template
/// (Authentication → Features → JWT Template) injects
/// `{"email": "{{ user.email }}"}` into every issued token. The API reads
/// that claim on `POST /v1/waitlist` so it does not need a second WorkOS
/// round-trip.
export function buildAuthorizeUrl(state: string): string {
  const url = new URL(config.web.authkit.authorize_url);
  url.searchParams.set("client_id", config.web.authkit.client_id);
  url.searchParams.set("redirect_uri", `${config.web.base_url}/auth/callback`);
  url.searchParams.set("response_type", "code");
  url.searchParams.set("scope", "openid profile email");
  url.searchParams.set("state", state);
  return url.toString();
}

/// Build the local-mode synthetic callback URL. Used when
/// `config.web.auth_mode === "bypass"` so the BFF can short-circuit
/// the AuthKit network call and bounce straight to its own `/auth/callback`.
export function buildLocalCallbackRedirect(state: string): string {
  const url = new URL(`${config.web.base_url}/auth/callback`);
  url.searchParams.set("code", "local-stub");
  url.searchParams.set("state", state);
  return url.toString();
}

function redirect(query: string): Response {
  const url = new URL(config.landing.base_url);
  url.search = query;
  return new Response(null, {
    status: 302,
    headers: {
      location: url.toString(),
      "set-cookie": clearCookieHeader(),
    },
  });
}

/// 302 redirect back to the landing page with `?waitlist=error&reason=<reason>`.
/// Always clears the state cookie.
export function buildFailureRedirect(reason: CallbackReason): Response {
  return redirect(`?waitlist=error&reason=${reason}`);
}

/// 302 redirect back to the landing page with `?waitlist=success`.
/// Always clears the state cookie.
export function buildSuccessRedirect(): Response {
  return redirect("?waitlist=success");
}
