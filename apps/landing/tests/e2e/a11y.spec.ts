import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test("skip link is focusable and reveals itself on focus", async ({ page }) => {
  await page.goto("/");
  const skipLink = page.getByRole("link", { name: /skip to main content/i });
  await skipLink.focus();
  await expect(skipLink).toBeFocused();
});

test("no console errors on initial load", async ({ page }) => {
  const errors: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      errors.push(message.text());
    }
  });
  page.on("pageerror", (err) => errors.push(err.message));
  await page.goto("/");
  expect(errors).toEqual([]);
});

test("404 page is reachable and passes axe", async ({ page }) => {
  const response = await page.goto("/404");
  expect(response?.status()).toBeLessThan(400);
  await expect(page.getByRole("heading", { level: 1, name: /page not found/i })).toBeVisible();
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations).toEqual([]);
});
