import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { OauthStatePayload } from "../../../src/utils/auth/cookies.server";

const SIGNING_KEY = "unit-test-signing-key";

beforeEach(() => {
  vi.stubEnv("TOKENOVERFLOW_ENV", "production");
  vi.resetModules();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

async function importModule() {
  return await import("../../../src/utils/auth/cookies.server");
}

const makePayload = async () => {
  const { generateState } = await importModule();
  return {
    state: generateState(),
    intent: "waitlist" as const,
  };
};

describe("signStateCookie / verifyStateCookie", () => {
  it("round-trips a payload with a matching state", async () => {
    const { signStateCookie, verifyStateCookie } = await importModule();
    const payload = await makePayload();
    const cookie = await signStateCookie(payload, SIGNING_KEY);
    const result = await verifyStateCookie({
      cookie_value: cookie,
      expected_state: payload.state,
      signing_key: SIGNING_KEY,
    });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.payload.state).toBe(payload.state);
      expect(result.payload.intent).toBe("waitlist");
    }
  });

  it("rejects a tampered signature", async () => {
    const { signStateCookie, verifyStateCookie } = await importModule();
    const payload = await makePayload();
    const cookie = await signStateCookie(payload, SIGNING_KEY);
    // Jose JWTs are three base64url segments separated by `.`. Replacing
    // the trailing signature segment with arbitrary bytes simulates a
    // tampered cookie.
    const [header, body] = cookie.split(".");
    const tampered = `${header}.${body}.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA`;
    const result = await verifyStateCookie({
      cookie_value: tampered,
      expected_state: payload.state,
      signing_key: SIGNING_KEY,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("bad_signature");
    }
  });

  it("rejects an expired payload", async () => {
    const { signStateCookie, verifyStateCookie } = await importModule();
    const payload = await makePayload();
    const cookie = await signStateCookie(payload, SIGNING_KEY);
    // Jose's `exp` is wall-clock; jumping `now` 11 minutes past issuance
    // simulates an expired token.
    const result = await verifyStateCookie({
      cookie_value: cookie,
      expected_state: payload.state,
      signing_key: SIGNING_KEY,
      now_unix_seconds: Math.floor(Date.now() / 1000) + 660,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("expired");
    }
  });

  it("rejects a state mismatch", async () => {
    const { signStateCookie, verifyStateCookie } = await importModule();
    const payload = await makePayload();
    const cookie = await signStateCookie(payload, SIGNING_KEY);
    const result = await verifyStateCookie({
      cookie_value: cookie,
      expected_state: "different-state",
      signing_key: SIGNING_KEY,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("state_mismatch");
    }
  });

  it("rejects a malformed cookie value", async () => {
    const { verifyStateCookie } = await importModule();
    const result = await verifyStateCookie({
      cookie_value: "not-a-cookie",
      expected_state: "any",
      signing_key: SIGNING_KEY,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("malformed");
    }
  });

  it("rejects a cookie with non-jwt segments", async () => {
    const { verifyStateCookie } = await importModule();
    const result = await verifyStateCookie({
      cookie_value: "@@@.@@@.@@@",
      expected_state: "any",
      signing_key: SIGNING_KEY,
    });
    expect(result.ok).toBe(false);
  });

  it("returns state_mismatch on length-equal-but-content-different inputs", async () => {
    const { signStateCookie, verifyStateCookie } = await importModule();
    // Same byte length, different content: this hits the
    // `timingSafeEqual` path (lengths align, comparison fails).
    const payload: OauthStatePayload = {
      state: "a".repeat(32),
      intent: "waitlist",
    };
    const cookie = await signStateCookie(payload, SIGNING_KEY);
    const result = await verifyStateCookie({
      cookie_value: cookie,
      expected_state: "b".repeat(32),
      signing_key: SIGNING_KEY,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("state_mismatch");
    }
  });

  it("returns state_mismatch on length-different inputs without throwing", async () => {
    const { signStateCookie, verifyStateCookie } = await importModule();
    // Mismatched byte length: the explicit length guard short-circuits
    // before `timingSafeEqual` (which would otherwise throw a TypeError
    // on length-different buffers).
    const payload: OauthStatePayload = {
      state: "a".repeat(32),
      intent: "waitlist",
    };
    const cookie = await signStateCookie(payload, SIGNING_KEY);
    const result = await verifyStateCookie({
      cookie_value: cookie,
      expected_state: "x",
      signing_key: SIGNING_KEY,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("state_mismatch");
    }
  });
});

describe("generateState", () => {
  it("produces a 43-char base64url token", async () => {
    const { generateState } = await importModule();
    const value = generateState();
    // 32 random bytes encoded as base64url = ceil(32 * 4 / 3) = 43 chars
    // (no padding, alphabet `[A-Za-z0-9_-]`).
    expect(value).toMatch(/^[A-Za-z0-9_-]{43}$/);
  });
});

describe("buildStateCookieHeader", () => {
  it("uses the __Host- prefixed name with all hardening attributes", async () => {
    const { buildStateCookieHeader, OAUTH_STATE_COOKIE_NAME } = await importModule();
    const header = buildStateCookieHeader("cookie-value");
    expect(OAUTH_STATE_COOKIE_NAME).toBe("__Host-oauth_state");
    expect(header).toContain(`${OAUTH_STATE_COOKIE_NAME}=cookie-value`);
    expect(header).toContain("Path=/");
    expect(header).toContain("HttpOnly");
    expect(header).toContain("SameSite=Lax");
    expect(header).toContain("Max-Age=600");
    // Secure is always emitted: localhost is a potentially-trustworthy
    // origin per the W3C Secure Contexts spec, so modern browsers honor
    // Secure cookies over HTTP-localhost.
    expect(header).toContain("Secure");
  });
});

describe("clearCookieHeader", () => {
  it("emits Max-Age=0 with the same hardening attributes", async () => {
    const { clearCookieHeader, OAUTH_STATE_COOKIE_NAME } = await importModule();
    const header = clearCookieHeader();
    expect(header).toContain(`${OAUTH_STATE_COOKIE_NAME}=`);
    expect(header).toContain("Max-Age=0");
    expect(header).toContain("Secure");
    expect(header).toContain("Path=/");
    expect(header).toContain("HttpOnly");
    expect(header).toContain("SameSite=Lax");
  });
});

describe("readStateCookie", () => {
  it("returns the named cookie value when present", async () => {
    const { readStateCookie, OAUTH_STATE_COOKIE_NAME } = await importModule();
    const request = new Request("https://app.example.com/", {
      headers: { cookie: `first=one; ${OAUTH_STATE_COOKIE_NAME}=signed.value.here; second=two` },
    });
    expect(readStateCookie(request)).toBe("signed.value.here");
  });

  it("returns undefined when the cookie header is missing", async () => {
    const { readStateCookie } = await importModule();
    const request = new Request("https://app.example.com/");
    expect(readStateCookie(request)).toBeUndefined();
  });

  it("returns undefined when the named cookie is absent", async () => {
    const { readStateCookie } = await importModule();
    const request = new Request("https://app.example.com/", {
      headers: { cookie: "first=one" },
    });
    expect(readStateCookie(request)).toBeUndefined();
  });
});
