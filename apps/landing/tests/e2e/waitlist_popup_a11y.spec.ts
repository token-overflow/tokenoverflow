import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

const reasons = [
  "success",
  "error&reason=oauth_denied",
  "error&reason=oauth_failed",
  "error&reason=state_invalid",
  "error&reason=server_error",
];

for (const reason of reasons) {
  test(`popup state ?waitlist=${reason} has zero axe violations`, async ({ page }) => {
    await page.goto(`/?waitlist=${reason}`);
    const results = await new AxeBuilder({ page }).analyze();
    expect(results.violations).toEqual([]);
  });
}
