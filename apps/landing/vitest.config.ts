import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["tests/unit/**/*.test.ts"],
    coverage: {
      // We stay on istanbul-lib-instrument. Vitest 4.1.5 added a
      // `coverage.instrumenter` hook that would let us swap in
      // oxc-coverage-instrument (20-24x faster), but oxc does not count
      // the "not-taken" side of an `if`-without-`else`:
      //
      //   if (x) { doA(); }    istanbul: 2 branches (taken + not-taken)
      //                        oxc:      1 branch  (taken only)
      //
      // A test that only feeds `x=true` reads 50% under istanbul (red,
      // correctly flagging the missing test for the not-taken path) and
      // 100% under oxc (silent). That is the exact signal this branch
      // gate exists to catch. The upstream maintainer flagged the
      // divergence as intentional in fallow-rs/oxc-coverage-instrument#9
      // "Category B", so it will not change.
      provider: "istanbul",
      reporter: ["text", "text-summary", "json-summary", "html"],
      reportsDirectory: "./coverage",
      include: ["src/lib/**/*.{ts,js}"],
      exclude: [
        "**/*.test.ts",
        "**/*.spec.ts",
        "**/dist/**",
        "**/.astro/**",
        "tests/**",
        "node_modules/**",
      ],
      thresholds: {
        lines: 95,
        functions: 95,
        branches: 95,
        statements: 95,
        autoUpdate: false,
      },
    },
  },
});
