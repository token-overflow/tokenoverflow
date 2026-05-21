import { expect, test } from "@playwright/test";

/// Cross-stack E2E spec: drives the real BFF running in the local docker
/// stack. Requires `docker compose up -d --build` to be running before
/// the test starts. The BFF auto-bypasses AuthKit because the `web`
/// service has `TOKENOVERFLOW_ENV=local`.

const LANDING_URL = "http://localhost:4321/";

test.describe("waitlist cross-stack", () => {
  test("CTA -> BFF -> API -> success popup", async ({ page }) => {
    await page.goto(LANDING_URL);

    const cta = page.getByRole("link", { name: /join the waitlist/i });
    await expect(cta).toBeVisible();

    // Follow the redirect chain by clicking the CTA. The BFF in local mode
    // bounces through `/auth/start` -> `/auth/callback` -> back to landing.
    await cta.click();

    await page.waitForURL(/\/\?waitlist=success/);

    const popup = page.locator("[data-waitlist-popup]");
    await expect(popup).toHaveAttribute("data-state", "visible");
    await expect(popup.locator("[data-waitlist-title]")).toContainText(/you're on the list/i);

    // Replace state should strip the param after the popup renders.
    await expect(page).toHaveURL(/\/$/);
  });

  test("second click is idempotent (popup still success)", async ({ page }) => {
    await page.goto(LANDING_URL);
    await page.getByRole("link", { name: /join the waitlist/i }).click();
    await page.waitForURL(/\/\?waitlist=success/);

    // Click again from the success state.
    await page.goto(LANDING_URL);
    await page.getByRole("link", { name: /join the waitlist/i }).click();
    await page.waitForURL(/\/\?waitlist=success/);

    await expect(page.locator("[data-waitlist-popup]")).toHaveAttribute("data-state", "visible");
  });
});
