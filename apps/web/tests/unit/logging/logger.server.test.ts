/// Tests for the pino + pino-lambda logger module.
///
/// We construct a minimal pino logger that mirrors the production
/// `redact` config and write into a `pino-test` sink so each line is
/// available as a structured JSON object. The production module's
/// `logger`/`createRequestLogger` exports are tested separately
/// (`request_logger.server.test.ts`); this file focuses on the
/// formatter, redact paths, and timestamp shape.

import pino from "pino";
import { once as pinoTestOnce, sink as pinoTestSink } from "pino-test";
import { describe, expect, it } from "vitest";

/// Build a logger configured the same way the production module does
/// (level-as-string, ISO-8601 timestamp, full redact path list) but
/// piped into a `pino-test` sink so the test owns the destination.
function buildTestLogger(): {
  logger: pino.Logger;
  stream: ReturnType<typeof pinoTestSink>;
} {
  const stream = pinoTestSink();
  const logger = pino(
    {
      level: "info",
      formatters: { level: (label) => ({ level: label }) },
      timestamp: pino.stdTimeFunctions.isoTime,
      redact: {
        paths: [
          "code",
          "*.code",
          "oauth.code",
          "state",
          "*.state",
          "oauth.state",
          "token",
          "*.token",
          "access_token",
          "*.access_token",
          "id_token",
          "*.id_token",
          "refresh_token",
          "*.refresh_token",
          "jwt",
          "*.jwt",
          "headers.authorization",
          "req.headers.authorization",
          "*.headers.authorization",
          "headers.cookie",
          "req.headers.cookie",
          "*.headers.cookie",
          "cookie",
          "*.cookie",
          "cookies.*",
          "client_secret",
          "*.client_secret",
          "cookie_signing_key",
          "*.cookie_signing_key",
        ],
        censor: "[redacted]",
      },
    },
    stream,
  );
  return { logger, stream };
}

/// Capture the next emitted line from the sink as a typed record. The
/// callback form of `pinoTestOnce` is the documented way to introspect
/// individual fields against arbitrary assertions.
async function nextLine(stream: ReturnType<typeof pinoTestSink>): Promise<Record<string, unknown>> {
  let captured: Record<string, unknown> | undefined;
  await pinoTestOnce(stream, (log: unknown) => {
    captured = log as Record<string, unknown>;
  });
  if (captured === undefined) {
    throw new Error("expected a log line, got none");
  }
  return captured;
}

describe("logger config", () => {
  it("emits the level as a string field, not a numeric severity", async () => {
    const { logger, stream } = buildTestLogger();
    logger.info("ts_event");
    const received = await nextLine(stream);
    expect(received["msg"]).toBe("ts_event");
    expect(received["level"]).toBe("info");
  });

  it("emits the timestamp as an ISO-8601 UTC string", () => {
    // Pino-test's `once`/`check` strips `time`/`pid`/`hostname` from
    // the captured chunk before invoking the callback, so we cannot
    // observe `time` through the sink. Verify the formatter shape
    // directly: `pino.stdTimeFunctions.isoTime` returns a string of
    // the form `,"time":"<ISO-8601>"` (the leading separator and the
    // wrapping JSON key are intentional, pino concatenates this
    // verbatim into the log line).
    const stamp = pino.stdTimeFunctions.isoTime();
    expect(stamp).toMatch(/^,"time":"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}.\d{3}Z"$/);
  });

  it("redacts OAuth code and state at top-level and nested paths", async () => {
    const { logger, stream } = buildTestLogger();
    const codeValue = "auth-code-123";
    const stateValue = "state-deadbeef";
    logger.info({ code: codeValue, oauth: { state: stateValue } }, "oauth_event");
    const received = await nextLine(stream);
    expect(received["code"]).toBe("[redacted]");
    expect((received["oauth"] as Record<string, unknown>)["state"]).toBe("[redacted]");
    // Whole-line backstop: the literal values must not appear anywhere.
    const raw = JSON.stringify(received);
    expect(raw).not.toContain(codeValue);
    expect(raw).not.toContain(stateValue);
  });

  it("redacts JWT carrier fields (access_token, id_token, refresh_token, jwt, token)", async () => {
    const { logger, stream } = buildTestLogger();
    const access = "access.eyJfoo.bar";
    const id = "id.eyJfoo.bar";
    const refresh = "refresh.eyJfoo.bar";
    const jwt = "raw.eyJfoo.bar";
    const token = "plain.eyJfoo.bar";
    logger.info(
      {
        access_token: access,
        id_token: id,
        refresh_token: refresh,
        jwt,
        token,
      },
      "tokens_event",
    );
    const received = await nextLine(stream);
    expect(received["access_token"]).toBe("[redacted]");
    expect(received["id_token"]).toBe("[redacted]");
    expect(received["refresh_token"]).toBe("[redacted]");
    expect(received["jwt"]).toBe("[redacted]");
    expect(received["token"]).toBe("[redacted]");
    const raw = JSON.stringify(received);
    expect(raw).not.toContain(access);
    expect(raw).not.toContain(id);
    expect(raw).not.toContain(refresh);
    expect(raw).not.toContain(jwt);
    expect(raw).not.toContain(token);
  });

  it("redacts authorization and cookie request headers", async () => {
    const { logger, stream } = buildTestLogger();
    const authValue = "Bearer eyJabc";
    const cookieValue = "__Host-oauth_state=eyJsign";
    logger.info(
      { headers: { authorization: authValue, cookie: cookieValue } },
      "request_headers_event",
    );
    const received = await nextLine(stream);
    const headers = received["headers"] as Record<string, unknown>;
    expect(headers["authorization"]).toBe("[redacted]");
    expect(headers["cookie"]).toBe("[redacted]");
    const raw = JSON.stringify(received);
    expect(raw).not.toContain(authValue);
    expect(raw).not.toContain(cookieValue);
  });

  it("redacts top-level cookie field and nested cookies.*", async () => {
    const { logger, stream } = buildTestLogger();
    const cookieValue = "raw-cookie-data";
    const stateCookie = "state-cookie-data";
    logger.info({ cookie: cookieValue, cookies: { state: stateCookie } }, "cookie_event");
    const received = await nextLine(stream);
    expect(received["cookie"]).toBe("[redacted]");
    expect((received["cookies"] as Record<string, unknown>)["state"]).toBe("[redacted]");
    const raw = JSON.stringify(received);
    expect(raw).not.toContain(cookieValue);
    expect(raw).not.toContain(stateCookie);
  });

  it("redacts server-side secrets: client_secret and cookie_signing_key", async () => {
    const { logger, stream } = buildTestLogger();
    const clientSecret = "super-secret-value";
    const signingKey = "cookie-signing-secret";
    logger.info({ client_secret: clientSecret, cookie_signing_key: signingKey }, "secrets_event");
    const received = await nextLine(stream);
    expect(received["client_secret"]).toBe("[redacted]");
    expect(received["cookie_signing_key"]).toBe("[redacted]");
    const raw = JSON.stringify(received);
    expect(raw).not.toContain(clientSecret);
    expect(raw).not.toContain(signingKey);
  });

  it("does not catch substrings inside `msg` (callers must not interpolate secrets)", async () => {
    // This documents pino's substring-redaction limitation. The redact
    // engine targets entire property values, so a token interpolated
    // into the message string is NOT scrubbed. Callers must pass
    // secrets as separate fields under known paths.
    const { logger, stream } = buildTestLogger();
    const leak = "leak.eyJfoo.bar";
    logger.info({}, `got token ${leak}`);
    const received = await nextLine(stream);
    expect(received["msg"]).toContain(leak);
  });
});
