import { describe, expect, it } from "vitest";

import { callApi } from "../../../src/utils/api/_call.server";

interface DummyData {
  hello: string;
}

function mockResponse(status: number): Response {
  return new Response(null, { status });
}

describe("callApi", () => {
  it("returns ok with data when the SDK resolves successfully", async () => {
    const result = await callApi<DummyData>(async () => ({
      data: { hello: "world" },
      error: undefined,
      response: mockResponse(201),
    }));
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data.hello).toBe("world");
    }
  });

  it("forwards an AbortSignal with a timeout to the inner fn", async () => {
    let captured: AbortSignal | undefined;
    await callApi<DummyData>(async (signal) => {
      captured = signal;
      return {
        data: { hello: "world" },
        error: undefined,
        response: mockResponse(201),
      };
    });
    // `AbortSignal.timeout(...)` returns an `AbortSignal` instance whose
    // `aborted` flips to true after the timeout fires; we can only check
    // the type here since the timer is far from elapsing in a unit test.
    expect(captured).toBeInstanceOf(AbortSignal);
  });

  it("returns reason=auth_rejected on 401", async () => {
    const result = await callApi<DummyData>(async () => ({
      data: undefined,
      error: {},
      response: mockResponse(401),
    }));
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("auth_rejected");
      expect(result.status).toBe(401);
    }
  });

  it("returns reason=client_error on a non-401 4xx", async () => {
    const result = await callApi<DummyData>(async () => ({
      data: undefined,
      error: {},
      response: mockResponse(422),
    }));
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("client_error");
      expect(result.status).toBe(422);
    }
  });

  it("returns reason=server_error on a 5xx", async () => {
    const result = await callApi<DummyData>(async () => ({
      data: undefined,
      error: {},
      response: mockResponse(503),
    }));
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("server_error");
      expect(result.status).toBe(503);
    }
  });

  it("returns reason=server_error when the SDK reports an error without a status", async () => {
    const result = await callApi<DummyData>(async () => ({
      data: undefined,
      error: {},
      response: undefined,
    }));
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("server_error");
    }
  });

  it("returns reason=malformed_response when both data and error are absent", async () => {
    const result = await callApi<DummyData>(async () => ({
      data: undefined,
      error: undefined,
      response: mockResponse(201),
    }));
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("malformed_response");
    }
  });

  it("returns reason=timeout when the inner fn rejects with an AbortSignal TimeoutError", async () => {
    const result = await callApi<DummyData>(async () => {
      throw new DOMException("timed out", "TimeoutError");
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("timeout");
      expect(result.status).toBeUndefined();
    }
  });

  it("returns reason=network_error on a generic rejection", async () => {
    const result = await callApi<DummyData>(async () => {
      throw new Error("dns failed");
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("network_error");
      expect(result.status).toBeUndefined();
    }
  });
});
