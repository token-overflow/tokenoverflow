/// Single-import surface for OAuth route handlers.
///
/// Each handler needs a per-request logger. Bundling the factory behind
/// one entry point keeps the route handler under the project's
/// import-budget cap and makes the handler's first line
/// self-documenting. The `request_id` field is mixed onto every line by
/// `pino-lambda`'s `lambdaRequestTracker` (wired from the Nitro plugin
/// at `apps/web/src/server/plugins/log_context.ts`); this factory only
/// owns the application-level `route` binding.

import { createRequestLogger, type RequestLogger } from "./request_logger.server";

export interface RouteLoggerArgs {
  /// Stable route identifier, e.g. `GET /auth/callback`.
  route: string;
}

/// Build the per-handler `RequestLogger` for an OAuth route. Stamps
/// `route` on every emitted line; `request_id` flows through
/// pino-lambda's per-invocation tracker.
export function startRequestLogger(args: RouteLoggerArgs): RequestLogger {
  return createRequestLogger({ route: args.route });
}
