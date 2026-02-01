import { defineConfig, devices } from "@playwright/test";
import { config } from "@tokenoverflow/config";

const baseURL = config.landing.base_url;
const isCI = Boolean(process.env["CI"]);

export default defineConfig({
  testDir: "./tests",
  testMatch: ["**/tests/e2e/**/*.spec.ts", "**/tests/integration/**/*.spec.ts"],
  fullyParallel: true,
  forbidOnly: true,
  retries: 0,
  workers: isCI ? 1 : "100%",
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL,
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
      testIgnore: ["**/tests/e2e/mobile.spec.ts"],
    },
    {
      name: "firefox",
      use: { ...devices["Desktop Firefox"] },
      testIgnore: ["**/tests/e2e/mobile.spec.ts"],
    },
    {
      name: "webkit",
      use: { ...devices["Desktop Safari"] },
      testIgnore: ["**/tests/e2e/mobile.spec.ts"],
    },
    {
      name: "webkit-mobile",
      use: { ...devices["iPhone 14"] },
      testMatch: ["**/tests/e2e/mobile.spec.ts"],
    },
  ],
});
