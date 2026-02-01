import { afterEach, describe, expect, it, vi } from "vitest";

describe("config loader", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.resetModules();
  });

  it("defaults to local when TOKENOVERFLOW_ENV is unset", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(config.env).toBe("local");
    expect(config.landing.base_url).toBe("http://localhost:4321");
  });

  it("returns production values when TOKENOVERFLOW_ENV=production", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "production");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(config.env).toBe("production");
    expect(config.landing.base_url).toBe("https://tokenoverflow.io");
  });

  it("throws a clear error when TOKENOVERFLOW_ENV is unknown", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "staging");
    vi.resetModules();

    await expect(import("../../src/index.js")).rejects.toThrow(
      /Invalid TOKENOVERFLOW_ENV=staging\. Expected one of: local, production\./,
    );
  });

  it("returns a frozen config object", async () => {
    vi.stubEnv("TOKENOVERFLOW_ENV", "local");
    vi.resetModules();
    const { config } = await import("../../src/index.js");

    expect(Object.isFrozen(config)).toBe(true);
  });

  it("schema rejects a malformed environment object", async () => {
    const { AppConfigSchema } = await import("../../src/schema.js");
    const { parse } = await import("valibot");

    expect(() =>
      parse(AppConfigSchema, {
        env: "local",
        landing: { base_url: "not-a-url" },
      }),
    ).toThrow();
  });
});
