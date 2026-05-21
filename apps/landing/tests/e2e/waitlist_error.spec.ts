import { expect, test } from "@playwright/test";

const cases = [
  {
    reason: "oauth_denied" as const,
    title_match: /no worries/i,
  },
  {
    reason: "oauth_failed" as const,
    title_match: /sign-in didn't go through/i,
  },
  {
    reason: "state_invalid" as const,
    title_match: /session expired/i,
  },
  {
    reason: "server_error" as const,
    title_match: /something went wrong/i,
  },
];

for (const c of cases) {
  test(`popup shows correct copy for ?waitlist=error&reason=${c.reason}`, async ({ page }) => {
    await page.goto(`/?waitlist=error&reason=${c.reason}`);
    const popup = page.locator("[data-waitlist-popup]");
    await expect(popup).toHaveAttribute("data-state", "visible");
    await expect(popup.locator("[data-waitlist-title]")).toContainText(c.title_match);
    await expect(page).toHaveURL(/\/$/);
  });
}
