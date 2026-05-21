import { safeParse } from "valibot";
import { describe, expect, it } from "vitest";

import { CallbackQuerySchema } from "../../../src/utils/auth/schemas";

describe("CallbackQuerySchema", () => {
  it("accepts a success query (code + state)", () => {
    const result = safeParse(CallbackQuerySchema, {
      code: "abc",
      state: "deadbeef",
    });
    expect(result.success).toBe(true);
  });

  it("accepts an error query (error + state + optional description)", () => {
    const result = safeParse(CallbackQuerySchema, {
      error: "access_denied",
      state: "deadbeef",
      error_description: "user clicked cancel",
    });
    expect(result.success).toBe(true);
  });

  it("rejects a query missing both code and error", () => {
    const result = safeParse(CallbackQuerySchema, {
      state: "deadbeef",
    });
    expect(result.success).toBe(false);
  });
});
