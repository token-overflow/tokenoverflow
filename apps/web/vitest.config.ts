import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["tests/unit/**/*.test.ts", "tests/integration/**/*.test.ts"],
    exclude: ["tests/e2e/**", "node_modules/**", ".output/**", ".nitro/**"],
    setupFiles: ["./vitest.setup.ts"],
    coverage: {
      provider: "istanbul",
      include: ["src/**/*.ts", "src/**/*.tsx"],
      exclude: ["src/routes/**", "src/router.tsx", "src/config.ts"],
    },
  },
});
