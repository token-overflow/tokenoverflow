/// Structured logger for the BFF.
///
/// pino + pino-lambda. In AWS Lambda, pino-lambda's destination writes
/// JSON lines synchronously to `process.stdout` from the main thread (no
/// worker, no async buffering); CloudWatch's native `@requestid` /
/// `@timestamp` parsers can correlate lines without a custom log query.
/// In `TOKENOVERFLOW_ENV=local` the logger prefers `pino-pretty` so
/// `vite dev` renders human-readable output; the bundled docker image
/// drops the optional dep at build time, so the same code path falls
/// back to plain JSON on `process.stdout` inside the container.
///
/// The level is emitted as a string (`"info"`, `"warn"`, `"error"`) and
/// the timestamp as ISO-8601 in UTC, matching the existing CloudWatch
/// Insights queries the BFF was wired against.
///
/// Redaction is configured at the pino layer for known sensitive paths
/// (OAuth code/state, JWT carriers, Authorization / Cookie headers, and
/// server-side secrets). Pino redacts whole-property values, not
/// substrings inside a string, so callers MUST pass secrets as separate
/// fields rather than interpolating them into the log message.

import { createRequire } from "node:module";

import pino, { type DestinationStream } from "pino";
import { lambdaRequestTracker, PinoLogFormatter, pinoLambdaDestination } from "pino-lambda";

const isLocal = process.env["TOKENOVERFLOW_ENV"] === "local";

/// Pino-pretty is a devDependency that is intentionally NOT copied into
/// the production container's runtime stage. The probe is intentionally
/// non-static (string variable, not a literal) so Vite/Rolldown does
/// not pull pino-pretty into the bundle.
function buildLocalDestination(): DestinationStream | undefined {
  const target = "pino-pretty";
  try {
    createRequire(import.meta.url).resolve(target);
  } catch {
    return undefined;
  }
  return pino.transport({ target, options: { colorize: true } });
}

const destination = isLocal
  ? buildLocalDestination()
  : pinoLambdaDestination({ formatter: new PinoLogFormatter() });

export const logger = pino(
  {
    level: "info",
    formatters: {
      // Emit `"level":"info"` instead of pino's default numeric severity.
      level: (label) => ({ level: label }),
    },
    timestamp: pino.stdTimeFunctions.isoTime,
    redact: {
      paths: [
        // OAuth code / state, however nested.
        "code",
        "*.code",
        "oauth.code",
        "state",
        "*.state",
        "oauth.state",
        // JWTs: every common carrier name, top-level and one level deep.
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
        // Authorization / cookie headers in any captured request payload.
        "headers.authorization",
        "req.headers.authorization",
        "*.headers.authorization",
        "headers.cookie",
        "req.headers.cookie",
        "*.headers.cookie",
        // Cookie value (top-level captures and parsed cookie objects).
        "cookie",
        "*.cookie",
        "cookies.*",
        // Server-side secrets that should never appear in a payload at all.
        "client_secret",
        "*.client_secret",
        "cookie_signing_key",
        "*.cookie_signing_key",
      ],
      censor: "[redacted]",
    },
  },
  destination,
);

/// Pino-lambda request tracker. Called once per Lambda invocation by
/// the Nitro request hook (`apps/web/src/server/plugins/log_context.ts`),
/// it stamps `request_id` (mirroring `awsRequestId`) onto every line the
/// container emits until the next invocation overwrites it.
export const withRequest = lambdaRequestTracker({
  requestMixin: (_event, context) => ({ request_id: context.awsRequestId }),
});

export type Outcome = "success" | "failure";

export interface TerminalArgs {
  /// Whether the handler is returning a success or a failure response.
  outcome: Outcome;
  /// Failure taxonomy, omitted on success.
  reason?: string;
  /// Auxiliary fields specific to the terminal event.
  [key: string]: unknown;
}

export interface RequestLogger {
  /// Emit a non-terminal log line carrying the per-request bindings.
  info: (event: string, payload?: Record<string, unknown>) => void;
  warn: (event: string, payload?: Record<string, unknown>) => void;
  error: (event: string, payload?: Record<string, unknown>) => void;
  /// Emit the single terminal line for the handler. Adds `outcome`,
  /// `latency_ms`, and `reason` (when provided). Success outcomes emit
  /// at `info`; failure outcomes emit at `warn`.
  terminal: (event: string, args: TerminalArgs) => void;
}

export interface CreateRequestLoggerArgs {
  /// Stable route identifier such as `GET /auth/callback`. Used as-is in
  /// the `route` field on every emitted line.
  route: string;
}

/// Build a per-handler logger. The clock starts when this is called, so
/// callers should invoke it as the first statement of the handler.
/// `request_id` is stamped on every line by the pino-lambda request
/// tracker; this factory only owns the application-level `route` field.
export function createRequestLogger(args: CreateRequestLoggerArgs): RequestLogger {
  const start = Date.now();
  const child = logger.child({ route: args.route });
  return {
    info: (event, payload = {}) => child.info(payload, event),
    warn: (event, payload = {}) => child.warn(payload, event),
    error: (event, payload = {}) => child.error(payload, event),
    terminal: (event, terminal_args) => {
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
}
