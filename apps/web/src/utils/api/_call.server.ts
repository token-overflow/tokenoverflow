/// Generic BFF -> API call wrapper. Threads timeout, error-shape, and
/// status-code-to-reason mapping that every endpoint client would
/// otherwise duplicate. Pair this with hey-api's generated SDK so each
/// endpoint module is a thin lambda passing through the typed call.

export type ApiResult<T> =
  | { ok: true; data: NonNullable<T> }
  | { ok: false; status: number | undefined; reason: ApiFailureReason };

export type ApiFailureReason =
  | "auth_rejected"
  | "client_error"
  | "server_error"
  | "timeout"
  | "network_error"
  | "malformed_response";

const API_TIMEOUT_MS = 10_000;

interface HeyApiResult<T> {
  data?: T | undefined;
  error?: unknown;
  response?: Response | undefined;
}

export async function callApi<T>(
  fn: (signal: AbortSignal) => Promise<HeyApiResult<T>>,
): Promise<ApiResult<T>> {
  let result: HeyApiResult<T>;
  try {
    result = await fn(AbortSignal.timeout(API_TIMEOUT_MS));
    // Hey-api can throw on network failure even with throwOnError=false
    // (e.g. an `AbortError` from `AbortSignal.timeout`). Local convention
    // names the WHATWG fetch error `err` to disambiguate from the schema
    // parser's `error` literal used elsewhere in the BFF.
    // oxlint-disable-next-line unicorn/catch-error-name
  } catch (err) {
    if (err instanceof DOMException && err.name === "TimeoutError") {
      return { ok: false, status: undefined, reason: "timeout" };
    }
    return { ok: false, status: undefined, reason: "network_error" };
  }
  const status = result.response?.status;
  if (result.error !== undefined) {
    if (status === 401) {
      return { ok: false, status, reason: "auth_rejected" };
    }
    if (status !== undefined && status >= 500) {
      return { ok: false, status, reason: "server_error" };
    }
    if (status !== undefined && status >= 400) {
      return { ok: false, status, reason: "client_error" };
    }
    return { ok: false, status, reason: "server_error" };
  }
  if (result.data === undefined || result.data === null) {
    return { ok: false, status, reason: "malformed_response" };
  }
  return { ok: true, data: result.data as NonNullable<T> };
}
