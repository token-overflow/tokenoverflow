import { defineConfig, devices } from "@playwright/test";

const baseURL = "http://localhost:3000";

export default defineConfig({
  testDir: "./tests",
  testMatch: ["**/tests/e2e/**/*.spec.ts"],
  fullyParallel: false,
  forbidOnly: true,
  retries: process.env["CI"] ? 1 : 0,
  workers: 1,
  reporter: [["list"], ["html", { open: "never" }]],
  globalSetup: "./tests/global_setup.ts",
  use: {
    baseURL,
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
