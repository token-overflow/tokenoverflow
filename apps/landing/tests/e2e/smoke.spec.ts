import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test("landing page renders the hero and the waitlist CTA", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle(/TokenOverflow/);
  await expect(page.getByRole("heading", { level: 1 })).toContainText("AI coding agents");
  await expect(page.getByRole("button", { name: /join the waitlist/i })).toBeVisible();
});

test("landing page has zero axe violations", async ({ page }) => {
  await page.goto("/");
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations).toEqual([]);
});
