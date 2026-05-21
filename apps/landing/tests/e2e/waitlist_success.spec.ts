import { expect, test } from "@playwright/test";

test("popup shows on ?waitlist=success and history strips the param", async ({ page }) => {
  await page.goto("/?waitlist=success");

  const popup = page.locator("[data-waitlist-popup]");
  await expect(popup).toHaveAttribute("data-state", "visible");
  await expect(popup.locator("[data-waitlist-title]")).toContainText(/you're on the list/i);

  // History.replaceState should have stripped the param.
  await expect(page).toHaveURL(/\/$/);
});
