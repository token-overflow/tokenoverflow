/// State cookie helpers: header construction, sign/verify, and the
/// random-token primitive.
///
/// The cookie wire format is a compact-serialization JWT (HS256) signed
/// by `jose`. The cookie name uses the `__Host-` prefix so the browser
/// enforces `Secure` + `Path=/` + no `Domain` automatically; we also set
/// each attribute explicitly so the BFF can defend itself when the prefix
/// is stripped at an edge proxy.

import { randomBytes, timingSafeEqual } from "node:crypto";

import { parse as parseCookieHeader, serialize as serializeCookie } from "cookie";
import { errors as joseErrors, jwtVerify, SignJWT } from "jose";

/// `__Host-` prefix gets free browser-enforced hardening: cookies under
/// this name are required to be `Secure`, `Path=/`, and have no `Domain`
/// attribute. We emit it unconditionally because modern Chromium and
/// Firefox treat `localhost` as a "potentially trustworthy origin" per
/// the W3C Secure Contexts spec, so `Secure` is honored over HTTP and
/// the prefix is accepted. The wire format is a compact-serialization
/// JWT signed with HS256.
const COOKIE_NAME = "__Host-oauth_state";
/// Cookie `Max-Age` and JWT `exp` must stay in sync. `jose` owns the
/// JWT-level TTL via `setExpirationTime`; this number drives the
/// cookie attribute the browser uses to discard the cookie at the same
/// instant the token would have expired.
const COOKIE_TTL_SECONDS = 600;
const JWT_TTL = "10m";

/// Shared cookie attributes. Both the set-and-clear headers use the
/// identical hardening flags; the only attribute that differs between
/// them is `maxAge`. Centralising the base prevents the two helpers from
/// drifting (e.g. one of them dropping `secure` by accident).
const COOKIE_BASE_OPTIONS = {
  path: "/" as const,
  httpOnly: true,
  sameSite: "lax" as const,
  secure: true,
};

export const OAUTH_STATE_COOKIE_NAME = COOKIE_NAME;

export interface OauthStatePayload {
  state: string;
  intent: "waitlist";
}

/// Result of verifying a cookie value. The discriminant matches the
/// Result-shape pattern used by the rest of the BFF utility modules.
export type VerifyResult =
  | { ok: true; payload: OauthStatePayload }
  | { ok: false; reason: VerifyFailureReason };

export type VerifyFailureReason = "malformed" | "bad_signature" | "expired" | "state_mismatch";

export interface VerifyStateCookieOptions {
  cookie_value: string;
  expected_state: string;
  signing_key: string;
  /// Override `now()` for deterministic tests. Forwarded to `jose.jwtVerify`
  /// via `currentDate` so claim validation uses the supplied wall clock.
  now_unix_seconds?: number;
}

const TEXT_ENCODER = new TextEncoder();
const MS_PER_SECOND = 1_000;

const toSecretKey = (signing_key: string): Uint8Array => TEXT_ENCODER.encode(signing_key);

/// Sign a state payload as an HS256 JWT. `jose` injects the `exp`
/// registered claim from `setExpirationTime`; callers supply only
/// the application-specific fields (`state`, `intent`).
export const signStateCookie = async (
  payload: OauthStatePayload,
  signing_key: string,
): Promise<string> =>
  await new SignJWT({ state: payload.state, intent: payload.intent })
    .setProtectedHeader({ alg: "HS256" })
    .setExpirationTime(JWT_TTL)
    .sign(toSecretKey(signing_key));

/// Verify a state cookie value against the expected `state` query
/// parameter. Returns a discriminated `VerifyResult`. jose verifies
/// the JWT signature and `exp`; we layer a constant-time compare on
/// top via `node:crypto.timingSafeEqual` because jose has no awareness
/// of the OAuth `state` query param contract.
export const verifyStateCookie = async (
  options: VerifyStateCookieOptions,
): Promise<VerifyResult> => {
  const { cookie_value, expected_state, signing_key, now_unix_seconds } = options;

  let claims: Record<string, unknown>;
  try {
    const verified = await jwtVerify(cookie_value, toSecretKey(signing_key), {
      algorithms: ["HS256"],
      ...(now_unix_seconds === undefined
        ? {}
        : { currentDate: new Date(now_unix_seconds * MS_PER_SECOND) }),
    });
    claims = verified.payload as Record<string, unknown>;
    // Local convention names the jose verifier error `err` to
    // disambiguate from the failure-reason `error` literals returned by
    // this module.
    // oxlint-disable-next-line unicorn/catch-error-name
  } catch (err) {
    if (err instanceof joseErrors.JWTExpired) {
      return { ok: false, reason: "expired" };
    }
    if (err instanceof joseErrors.JWSSignatureVerificationFailed) {
      return { ok: false, reason: "bad_signature" };
    }
    if (err instanceof joseErrors.JWTInvalid || err instanceof joseErrors.JWSInvalid) {
      return { ok: false, reason: "malformed" };
    }
    // Anything else (including base64 parse failures inside jose) is a
    // malformed cookie from our perspective; we never want to leak the
    // underlying error to the user.
    return { ok: false, reason: "malformed" };
  }

  const state_claim = claims["state"];
  if (typeof state_claim !== "string" || claims["intent"] !== "waitlist") {
    return { ok: false, reason: "malformed" };
  }

  const expected_bytes = TEXT_ENCODER.encode(expected_state);
  const claim_bytes = TEXT_ENCODER.encode(state_claim);
  // `timingSafeEqual` throws when the buffers differ in length; the
  // explicit length guard keeps the function returning the documented
  // `state_mismatch` reason on length-mismatched inputs instead of
  // surfacing the underlying TypeError.
  if (
    expected_bytes.byteLength !== claim_bytes.byteLength ||
    !timingSafeEqual(expected_bytes, claim_bytes)
  ) {
    return { ok: false, reason: "state_mismatch" };
  }

  return { ok: true, payload: { state: state_claim, intent: "waitlist" } };
};

/// Generate a 32-byte random token suitable for the OAuth `state` field,
/// encoded as a 43-char base64url string.
export const generateState = (): string => randomBytes(32).toString("base64url");

/// Build the `Set-Cookie` header value for the freshly minted state
/// cookie. `Secure` is always set; localhost is a potentially-trustworthy
/// origin per the W3C Secure Contexts spec, so modern browsers honor
/// `Secure` cookies over plain HTTP-localhost.
export const buildStateCookieHeader = (value: string): string =>
  serializeCookie(COOKIE_NAME, value, { ...COOKIE_BASE_OPTIONS, maxAge: COOKIE_TTL_SECONDS });

/// Build the `Set-Cookie` header value that clears the state cookie.
/// Same flags as `buildStateCookieHeader` minus the value, with
/// `Max-Age=0` so user agents drop it immediately.
export const clearCookieHeader = (): string =>
  serializeCookie(COOKIE_NAME, "", { ...COOKIE_BASE_OPTIONS, maxAge: 0 });

/// Read the OAuth state cookie value out of an incoming `Request`.
/// Returns `undefined` if the cookie header is missing or the named
/// cookie is absent.
export function readStateCookie(request: Request): string | undefined {
  const cookie_header = request.headers.get("cookie");
  if (!cookie_header) {
    return undefined;
  }
  const parsed = parseCookieHeader(cookie_header);
  return parsed[COOKIE_NAME];
}
