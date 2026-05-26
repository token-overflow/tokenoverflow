import { defineConfig, devices } from "@playwright/test";

const baseURL = "http://localhost:3000";
const isCI = Boolean(process.env["CI"]);

export default defineConfig({
  testDir: "./tests",
  testMatch: ["**/tests/e2e/**/*.spec.ts"],
  fullyParallel: false,
  forbidOnly: true,
  retries: isCI ? 1 : 0,
  workers: 1,
  reporter: [["list"], ["html", { open: "never" }]],
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
  webServer: {
    cwd: "../..",
    command: "docker compose up -d --build --wait",
    url: "http://127.0.0.1:3000/health",
    reuseExistingServer: true,
    stdout: "pipe",
    stderr: "pipe",
    timeout: 600_000,
  },
});
