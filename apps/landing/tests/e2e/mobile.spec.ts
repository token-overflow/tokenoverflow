import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

// This spec runs only under the `chromium-mobile` project in
// playwright.config.ts. Playwright's `iPhone 14` descriptor wraps Chromium
// with mobile viewport and touch emulation; there is no WebKit dependency.
// The assertions mirror the desktop smoke + a11y checks at a smaller viewport
// and cover two mobile-specific concerns: no horizontal scrollbar and the
// footer is reachable via scroll.

test("hero and CTA are visible on a mobile viewport", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle(/TokenOverflow/);
  await expect(page.getByRole("heading", { level: 1 })).toContainText("AI coding agents");
  await expect(page.getByRole("button", { name: /join the waitlist/i })).toBeVisible();
});

test("no horizontal scrollbar: scrollWidth equals clientWidth on <html>", async ({ page }) => {
  await page.goto("/");
  // Evaluate in the browser so we read the live layout values. A 1px
  // difference is tolerated because subpixel rounding on some Chromium
  // builds emits an off-by-one even on well-behaved layouts.
  const overflow = await page.evaluate(() => {
    const doc = document.documentElement;
    return { scrollWidth: doc.scrollWidth, clientWidth: doc.clientWidth };
  });
  expect(
    overflow.scrollWidth,
    `html.scrollWidth (${overflow.scrollWidth}) must equal html.clientWidth (${overflow.clientWidth})`,
  ).toBeLessThanOrEqual(overflow.clientWidth + 1);
});

test("footer is reachable by scrolling to the bottom", async ({ page }) => {
  await page.goto("/");
  const footer = page.locator("footer");
  await footer.scrollIntoViewIfNeeded();
  await expect(footer).toBeVisible();
});

test("axe scan has zero violations on mobile", async ({ page }) => {
  await page.goto("/");
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations).toEqual([]);
});

// Regression: on a single-column mobile layout, switching pipeline layers
// changes the panel height (different descriptions wrap differently). The
// shield in shield.astro must stay put across layer changes; previously it
// was anchored to a percentage of #security's height and visibly drifted.
test("shield does not drift when switching pipeline layers", async ({ page }) => {
  await page.goto("/");
  const tabs = page.getByRole("tab");
  await expect(tabs).toHaveCount(6);

  const tops: number[] = [];
  const tabCount = await tabs.count();
  for (let i = 0; i < tabCount; i++) {
    await tabs.nth(i).click();
    const top = await page.evaluate(() => {
      const shield = document.querySelector(".shield-deco");
      if (!shield) {
        throw new Error("shield-deco not found");
      }
      return shield.getBoundingClientRect().top + window.scrollY;
    });
    tops.push(top);
  }
  const maxDelta = Math.max(...tops) - Math.min(...tops);
  expect(
    maxDelta,
    `shield drifted ${maxDelta}px across layers ${JSON.stringify(tops)}`,
  ).toBeLessThanOrEqual(1);
});
