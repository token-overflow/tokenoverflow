import { config } from "@tokenoverflow/config";
import { expect, test } from "@playwright/test";

test("waitlist CTA links to the BFF /auth/start endpoint", async ({ page }) => {
  await page.goto("/");
  const cta = page.getByRole("link", { name: /join the waitlist/i });
  await expect(cta).toBeVisible();
  const href = await cta.getAttribute("href");
  expect(href).toBe(`${config.web.base_url}/auth/start?intent=waitlist`);
});
