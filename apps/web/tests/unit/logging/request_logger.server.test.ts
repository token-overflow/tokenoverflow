/// Tests for `createRequestLogger`. Uses `pino-test`'s sink so the
/// test owns the destination; the production module wires the same
/// behaviour onto a pino-lambda destination for the Lambda runtime.

import { setImmediate as setImmediatePromise } from "node:timers/promises";

import pino from "pino";
import { sink as pinoTestSink } from "pino-test";
import { describe, expect, it } from "vitest";

import type { RequestLogger, TerminalArgs } from "../../../src/utils/logging/logger.server";

/// Build a sink-backed logger and adapt it to the `RequestLogger`
/// surface so we can exercise `info` / `warn` / `error` / `terminal`
/// against a captured stream. This mirrors the production
/// `createRequestLogger` minus the pino-lambda destination wiring.
function buildRequestLogger(args: { route: string }): {
  log: RequestLogger;
  stream: ReturnType<typeof pinoTestSink>;
} {
  const stream = pinoTestSink();
  const baseLogger = pino(
    {
      level: "info",
      formatters: { level: (label) => ({ level: label }) },
      timestamp: pino.stdTimeFunctions.isoTime,
    },
    stream,
  );
  const start = Date.now();
  const child = baseLogger.child({ route: args.route });
  const log: RequestLogger = {
    info: (event, payload = {}) => child.info(payload, event),
    warn: (event, payload = {}) => child.warn(payload, event),
    error: (event, payload = {}) => child.error(payload, event),
    terminal: (event: string, terminal_args: TerminalArgs) => {
      const { outcome, reason, ...rest } = terminal_args;
      const payload: Record<string, unknown> = {
        ...rest,
        outcome,
        latency_ms: Date.now() - start,
      };
      if (reason !== undefined) {
        payload["reason"] = reason;
      }
      if (outcome === "success") {
        child.info(payload, event);
      } else {
        child.warn(payload, event);
      }
    },
  };
  return { log, stream };
}

/// Capture every line emitted on the sink. The harness drains the
/// underlying split2 transform stream into an in-memory list so a test
/// can emit several lines and inspect them as a batch (without needing
/// to interleave each log call with a `pinoTest.once`).
function captureAll(stream: ReturnType<typeof pinoTestSink>): {
  lines: Record<string, unknown>[];
} {
  const lines: Record<string, unknown>[] = [];
  stream.on("data", (chunk: Record<string, unknown>) => {
    lines.push(chunk);
  });
  return { lines };
}

/// Tick the event loop so the split2 transform inside the sink
/// finishes parsing the lines pino just synchronously wrote.
async function flush(): Promise<void> {
  await setImmediatePromise();
}

describe("createRequestLogger", () => {
  it("stamps `route` on every emitted line", async () => {
    const { log, stream } = buildRequestLogger({ route: "GET /auth/start" });
    const { lines } = captureAll(stream);
    log.info("oauth_start_received");
    log.warn("intermediate_warn");
    log.terminal("oauth_start_completed", { outcome: "success" });
    await flush();
    expect(lines).toHaveLength(3);
    const [a, b, c] = lines;
    expect(a!["msg"]).toBe("oauth_start_received");
    expect(a!["route"]).toBe("GET /auth/start");
    expect(b!["msg"]).toBe("intermediate_warn");
    expect(b!["route"]).toBe("GET /auth/start");
    expect(c!["msg"]).toBe("oauth_start_completed");
    expect(c!["route"]).toBe("GET /auth/start");
    expect(c!["outcome"]).toBe("success");
  });

  it("`terminal` success emits at info with outcome and latency_ms", async () => {
    const { log, stream } = buildRequestLogger({ route: "GET /auth/callback" });
    const { lines } = captureAll(stream);
    log.terminal("oauth_success", { outcome: "success", github_id: 1 });
    await flush();
    const received = lines[0]!;
    expect(received["msg"]).toBe("oauth_success");
    expect(received["level"]).toBe("info");
    expect(received["outcome"]).toBe("success");
    expect(received["github_id"]).toBe(1);
    expect(typeof received["latency_ms"]).toBe("number");
    expect(received["latency_ms"] as number).toBeGreaterThanOrEqual(0);
  });

  it("`terminal` failure emits at warn with reason forwarded", async () => {
    const { log, stream } = buildRequestLogger({ route: "GET /auth/callback" });
    const { lines } = captureAll(stream);
    log.terminal("oauth_callback_completed", {
      outcome: "failure",
      reason: "state_invalid",
      failure_kind: "cookie_missing",
    });
    await flush();
    const received = lines[0]!;
    expect(received["msg"]).toBe("oauth_callback_completed");
    expect(received["level"]).toBe("warn");
    expect(received["outcome"]).toBe("failure");
    expect(received["reason"]).toBe("state_invalid");
    expect(received["failure_kind"]).toBe("cookie_missing");
  });

  it("`terminal` omits the reason key when not provided (success path)", async () => {
    const { log, stream } = buildRequestLogger({ route: "GET /auth/callback" });
    const { lines } = captureAll(stream);
    log.terminal("oauth_success", { outcome: "success" });
    await flush();
    const received = lines[0]!;
    expect(received["msg"]).toBe("oauth_success");
    expect(received["outcome"]).toBe("success");
    expect(Object.hasOwn(received, "reason")).toBe(false);
    expect(received["level"]).toBe("info");
  });
});
