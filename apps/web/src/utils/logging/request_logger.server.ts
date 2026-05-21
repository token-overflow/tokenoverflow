/// Per-request logger surface re-exported for call sites.
///
/// `RequestLogger` is the type service modules (e.g. `authkit.server.ts`,
/// `strategy.server.ts`) accept on their `log:` argument. Each OAuth
/// route handler captures one of these at entry: it stamps `route` on
/// every line and `request_id` is mixed in by pino-lambda's
/// `lambdaRequestTracker`. Intermediate branches use the `info` / `warn`
/// / `error` helpers to surface a step-level event without the latency
/// stamp, and `terminal({ outcome, reason, ... })` is the single helper
/// the handler invokes immediately before returning a redirect.
///
/// There is one terminal log line per handler invocation by
/// construction (each branch ends with a `log.terminal(...)` call
/// followed by `return ...`).

export {
  createRequestLogger,
  type CreateRequestLoggerArgs,
  type Outcome,
  type RequestLogger,
  type TerminalArgs,
} from "./logger.server";
