/// Integration-test helper that seeds pino-lambda's per-invocation
/// request id without booting the full Nitro server.
///
/// In production, the Nitro plugin at
/// `apps/web/src/server/plugins/log_context.ts` calls `withRequest`
/// once per Lambda invocation. Integration tests bypass the Nitro
/// plugin (they import the route handler directly and call
/// `handler({ request })`), so the test must call `withRequest` itself
/// before invoking the handler. This helper centralises that wiring.

/// Seed the pino-lambda tracker with the `awsRequestId` derived from
/// the inbound request's `x-amzn-requestid` header. Subsequent log
/// lines emitted on the same Node process will carry that value as
/// `request_id` until the next call overwrites it.
export async function seedRequestId(request: Request): Promise<void> {
  const headerId = request.headers.get("x-amzn-requestid");
  if (headerId === null) {
    return;
  }
  const { withRequest } = await import("../../src/utils/logging/logger.server");
  withRequest(
    { headers: { "x-amzn-requestid": headerId } } as never,
    { awsRequestId: headerId } as never,
  );
}
