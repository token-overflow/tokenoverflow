import { expect, test } from "@playwright/test";

// Smooth-scroll delegate source: apps/landing/src/layouts/base.astro. It
// intercepts clicks on `a[href^='#']`, resolves the target, and calls
// scrollIntoView(). Under default motion the behaviour is 'smooth'; under
// reduced motion it is 'auto'. The result is the same: the target lands
// near viewport top, within the scroll-margin-top tolerance.

// scroll-margin-top on .section is 4rem = 64px; allow headroom for the
// sticky header (16rem = 64px) plus a small rounding buffer.
const TOLERANCE_PX = 96;

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
});

test("clicking an in-page nav anchor scrolls the target near viewport top", async ({ page }) => {
  await page.goto("/");

  // Pick the header's Demo link which points at #demo. Using getByRole
  // keeps this resilient if the header markup changes.
  const demoLink = page.getByRole("link", { name: /^demo$/i }).first();
  const hash = (await demoLink.getAttribute("href")) ?? "";
  expect(hash).toMatch(/^#/);

  const initialY = await page.evaluate(() => window.scrollY);

  await demoLink.click();

  // Wait for scrollY to settle; scroll-behavior: smooth would finish within
  // ~300ms but under test.use(reducedMotion: "reduce") the delegate uses
  // 'auto' so it should settle on the next frame.
  await page.waitForFunction((start) => window.scrollY > start, initialY);

  const scrollY = await page.evaluate(() => window.scrollY);
  expect(scrollY, "scrollY must have meaningfully advanced").toBeGreaterThan(initialY + 100);

  // The target's top edge must now sit near the top of the viewport after
  // accounting for scroll-margin-top.
  const targetTop = await page.evaluate((sel) => {
    const node = document.querySelector(sel);
    if (!node) {
      return Number.NaN;
    }
    return node.getBoundingClientRect().top;
  }, hash);

  expect(Number.isFinite(targetTop), "target node must exist in DOM").toBe(true);
  expect(
    Math.abs(targetTop),
    `target top (${targetTop}px) must be near viewport top within ${TOLERANCE_PX}px`,
  ).toBeLessThanOrEqual(TOLERANCE_PX);
});
