import { config } from "@tokenoverflow/config";
import { number, object, optional, safeParse, string } from "valibot";

import type { RequestLogger } from "../logging/request_logger.server";

/// Cap on the upstream error message we copy into the log line. Token-
/// exchange errors are sourced from the OAuth provider's
/// `error_description`, which is upstream-controlled; cap to bound
/// exposure if a provider ever echoes secrets there. Also caps the body
/// snippet on the non-JSON fallback so the log line stays short.
const ERROR_MESSAGE_MAX_CHARS = 200;

export interface ExchangeCodeArgs {
  code: string;
  /// Per-request logger. Failures are emitted through this so the
  /// `oauth_token_exchange_failed` line carries `request_id` and
  /// `route` from the calling handler's context.
  log: RequestLogger;
}

export type ExchangeCodeResult = { ok: true; access_token: string } | { ok: false };

/// Strict shape of AuthKit's `/oauth2/token` response. `token_type` and
/// `expires_in` are documented as always-present in the OAuth 2.0 spec,
/// but we mark them optional defensively because we only need
/// `access_token`. A literal `object({...})` (not `looseObject`) means
/// any drift from the documented shape fails closed; tracked via the
/// `malformed_response` log branch.
const AuthKitTokenResponseSchema = object({
  access_token: string(),
  token_type: optional(string()),
  expires_in: optional(number()),
});

/// Build the `oauth_token_exchange_failed` payload for a non-2xx
/// response. Reads the body as text, attempts a JSON parse to surface
/// `error` / `error_description`, and falls back to a capped raw-text
/// snippet when the body is not JSON. Returns the structured payload
/// the logger consumes.
async function describeNon2xx(response: Response): Promise<Record<string, unknown>> {
  const text_body = await response.text();
  const payload: Record<string, unknown> = {
    error_name: "non_2xx",
    error_status: response.status,
  };
  let parsed: { error?: unknown; error_description?: unknown } | undefined;
  try {
    parsed = JSON.parse(text_body) as { error?: unknown; error_description?: unknown };
  } catch (error) {
    // The non-2xx body might not be JSON; fall back to the raw text
    // (capped to the same bound) so ops still sees something
    // correlatable.
    void error;
  }
  if (parsed === undefined) {
    payload["error_message"] = text_body.slice(0, ERROR_MESSAGE_MAX_CHARS);
    return payload;
  }
  if (typeof parsed.error === "string") {
    payload["error_code"] = parsed.error;
  }
  if (typeof parsed.error_description === "string") {
    // `error_description` is upstream-controlled; cap so an exotic
    // provider message can't blow up the log line.
    payload["error_message"] = parsed.error_description.slice(0, ERROR_MESSAGE_MAX_CHARS);
  }
  return payload;
}

/// Exchange the OAuth `code` for a WorkOS-issued access token via a raw
/// `fetch` against `${config.web.authkit.token_url}`. We talk to the
/// per-app `/oauth2/token` endpoint with confidential-client credentials
/// (`client_id` + `client_secret`), which the WorkOS Node SDK does not
/// expose; the SDK's `userManagement.authenticateWithCode` posts to the
/// env-level User Management endpoint instead, which rejects per-app
/// `client_id`s with `invalid_client`.
///
/// On failure the structured error fields land in a single
/// `oauth_token_exchange_failed` log line emitted through the supplied
/// per-request `RequestLogger` so the line carries `request_id` and
/// `route` from the calling context. The caller still sees a plain
/// `{ ok: false }` so it does not need to branch on failure shape.
export async function exchangeCode({ code, log }: ExchangeCodeArgs): Promise<ExchangeCodeResult> {
  const body = new URLSearchParams({
    grant_type: "authorization_code",
    code,
    client_id: config.web.authkit.client_id,
    client_secret: config.web.authkit.client_secret,
    redirect_uri: `${config.web.base_url}/auth/callback`,
  });

  let response: Response;
  try {
    response = await fetch(config.web.authkit.token_url, {
      method: "POST",
      headers: {
        "content-type": "application/x-www-form-urlencoded",
        accept: "application/json",
      },
      body,
    });
  } catch (error) {
    log.warn("oauth_token_exchange_failed", {
      error_name: "fetch_failed",
      error_message:
        error instanceof Error
          ? error.message.slice(0, ERROR_MESSAGE_MAX_CHARS)
          : String(error).slice(0, ERROR_MESSAGE_MAX_CHARS),
    });
    return { ok: false };
  }

  if (!response.ok) {
    log.warn("oauth_token_exchange_failed", await describeNon2xx(response));
    return { ok: false };
  }

  const { status } = response;
  let payload: unknown;
  try {
    payload = await response.json();
  } catch (error) {
    // Body is upstream-controlled; treat both invalid JSON and unexpected
    // shapes uniformly via the `malformed_response` branch below.
    void error;
    log.warn("oauth_token_exchange_failed", {
      error_name: "malformed_response",
      error_status: status,
    });
    return { ok: false };
  }

  const parsed = safeParse(AuthKitTokenResponseSchema, payload);
  if (!parsed.success) {
    log.warn("oauth_token_exchange_failed", {
      error_name: "malformed_response",
      error_status: status,
    });
    return { ok: false };
  }

  return { ok: true, access_token: parsed.output.access_token };
}
