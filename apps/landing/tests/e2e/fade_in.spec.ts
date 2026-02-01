import { expect, test } from "@playwright/test";

import { computedOpacity } from "../common/motion";

// Fade-in observer source: apps/landing/src/layouts/base.astro (inline
// script that adds .is-visible when [data-fade-in] intersects). The
// transition CSS lives in apps/landing/src/styles/global.css.
//
// Shape of the assertions:
//   1. [data-fade-in] selector matches something on the rendered page.
//   2. Before scrolling, the bottom elements are at opacity < 1.
//   3. After scrolling, opacity resolves to exactly 1.

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
});

test("at least one [data-fade-in] element exists on /", async ({ page }) => {
  await page.goto("/");
  const count = await page.locator("[data-fade-in]").count();
  expect(count, "page must carry at least one [data-fade-in] element").toBeGreaterThan(0);
});

test("below-the-fold fade-in elements start below opacity 1 and reach 1 after scrolling", async ({
  page,
}) => {
  await page.goto("/");

  // Shield/pipeline section is well below the fold at every viewport we
  // support; its [data-fade-in] wrappers are the most stable target.
  const selector = "#security [data-fade-in]";
  const locator = page.locator(selector).first();
  await expect(locator, "security section must carry a fade-in")
    .toHaveCount(1, {
      timeout: 1_000,
    })
    .catch(async () => {
      // Soften to `attached` so a missing match fails with a clearer message.
      await expect(locator).toBeAttached();
    });

  const before = await computedOpacity(page, selector);
  expect(Number(before), "below-the-fold element must start below opacity 1").toBeLessThan(1);

  await locator.scrollIntoViewIfNeeded();
  await expect
    .poll(async () => Number(await computedOpacity(page, selector)), {
      timeout: 3_000,
      message: "fade-in must resolve to opacity 1 after scroll",
    })
    .toBe(1);
});
