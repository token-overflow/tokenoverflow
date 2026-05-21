import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const generatedAddToWaitlistMock = vi.fn();

/// Mock the generated hey-api SDK at module load. The wrapper at
/// `api/waitlist.server.ts` re-exports a thin Result-shaped function on
/// top of this generated client; replacing the underlying call lets us
/// drive every status-code branch through `generatedAddToWaitlistMock`.
vi.mock("../../../src/utils/api/_generated/sdk.gen", () => ({
  addToWaitlist: generatedAddToWaitlistMock,
}));

beforeEach(() => {
  vi.stubEnv("TOKENOVERFLOW_ENV", "production");
  vi.resetModules();
  generatedAddToWaitlistMock.mockReset();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

const successBody = {
  github_id: 99001,
  github_username: "octocat",
  already_on_waitlist: false,
};

function mockResponse(status: number): Response {
  return new Response(null, { status });
}

describe("addToWaitlist", () => {
  it("returns ok on a 201 with a valid body", async () => {
    generatedAddToWaitlistMock.mockResolvedValue({
      data: successBody,
      error: undefined,
      response: mockResponse(201),
      request: undefined,
    });
    const { addToWaitlist } = await import("../../../src/utils/api/waitlist.server");
    const result = await addToWaitlist({ workos_jwt: "jwt" });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data.github_id).toBe(99001);
      expect(result.data.github_username).toBe("octocat");
      expect(result.data.already_on_waitlist).toBe(false);
    }
    expect(generatedAddToWaitlistMock).toHaveBeenCalledTimes(1);
    const [arg] = generatedAddToWaitlistMock.mock.calls[0]!;
    expect(arg.baseUrl).toBe("https://pmbh5f29x3.execute-api.us-east-1.amazonaws.com");
    expect(arg.headers["authorization"]).toBe("Bearer jwt");
  });

  it("returns reason=auth_rejected on 401", async () => {
    generatedAddToWaitlistMock.mockResolvedValue({
      data: undefined,
      error: {},
      response: mockResponse(401),
      request: undefined,
    });
    const { addToWaitlist } = await import("../../../src/utils/api/waitlist.server");
    const result = await addToWaitlist({ workos_jwt: "jwt" });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("auth_rejected");
      expect(result.status).toBe(401);
    }
  });

  it("returns reason=client_error on a non-401 4xx", async () => {
    generatedAddToWaitlistMock.mockResolvedValue({
      data: undefined,
      error: {},
      response: mockResponse(422),
      request: undefined,
    });
    const { addToWaitlist } = await import("../../../src/utils/api/waitlist.server");
    const result = await addToWaitlist({ workos_jwt: "jwt" });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("client_error");
    }
  });

  it("returns reason=server_error on 5xx", async () => {
    generatedAddToWaitlistMock.mockResolvedValue({
      data: undefined,
      error: {},
      response: mockResponse(503),
      request: undefined,
    });
    const { addToWaitlist } = await import("../../../src/utils/api/waitlist.server");
    const result = await addToWaitlist({ workos_jwt: "jwt" });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("server_error");
    }
  });

  it("returns reason=malformed_response when both data and error are absent", async () => {
    generatedAddToWaitlistMock.mockResolvedValue({
      data: undefined,
      error: undefined,
      response: mockResponse(201),
      request: undefined,
    });
    const { addToWaitlist } = await import("../../../src/utils/api/waitlist.server");
    const result = await addToWaitlist({ workos_jwt: "jwt" });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("malformed_response");
    }
  });

  it("returns reason=timeout when AbortSignal.timeout fires", async () => {
    generatedAddToWaitlistMock.mockRejectedValue(new DOMException("timed out", "TimeoutError"));
    const { addToWaitlist } = await import("../../../src/utils/api/waitlist.server");
    const result = await addToWaitlist({ workos_jwt: "jwt" });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("timeout");
    }
  });

  it("returns reason=network_error on a generic SDK rejection", async () => {
    generatedAddToWaitlistMock.mockRejectedValue(new Error("dns failed"));
    const { addToWaitlist } = await import("../../../src/utils/api/waitlist.server");
    const result = await addToWaitlist({ workos_jwt: "jwt" });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.reason).toBe("network_error");
    }
  });
});
