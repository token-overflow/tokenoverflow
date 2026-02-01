import { expect, test } from "@playwright/test";

import { computedOpacity, useDefaultMotion, useReducedMotion } from "../common/motion";

// The landing page's motion contract lives across four behaviours:
//   - Terminal typer (apps/landing/src/components/terminal.astro)
//       -> renders final state immediately under reduce
//   - ASCII canvas (apps/landing/src/components/ascii.astro)
//       -> draws one frame; no requestAnimationFrame loop
//   - Pipeline tablist (apps/landing/src/components/pipeline.astro)
//       -> no auto-advance setInterval
//   - Fade-in observer (apps/landing/src/layouts/base.astro)
//       -> CSS @media override drops opacity to 1
//
// Each assertion carries a human-readable diagnostic message so the failure
// points at which contract regressed.

// A paused ASCII canvas must not update its pixels; we sample the first
// 4 x 40 pixel strip, which is enough to cover multiple glyphs but keeps
// the snapshot small enough that the serialisation is cheap.
const CANVAS_SAMPLE_WIDTH = 40;
const CANVAS_SAMPLE_HEIGHT = 4;

test.describe("prefers-reduced-motion: reduce", () => {
  test.beforeEach(async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
  });

  test("terminal shows final typed content immediately (no timed reveal)", async ({ page }) => {
    await useReducedMotion(page);
    await page.goto("/");

    const termLines = page.locator("#term-lines");
    await expect(termLines).toBeVisible();

    // Give the page a settle window; then two snapshots 200ms apart must be
    // identical. The typer script normally mutates the DOM on an interval,
    // so an identical length across the window proves the loop is not running.
    await page.waitForTimeout(200);
    const first = (await termLines.textContent()) ?? "";
    await page.waitForTimeout(200);
    const second = (await termLines.textContent()) ?? "";

    expect(first.length, "terminal text must be stable under reduced motion").toBe(second.length);
    expect(first.length).toBeGreaterThan(0);
  });

  test("ASCII canvas renders exactly one static frame", async ({ page }) => {
    await useReducedMotion(page);
    await page.goto("/");

    // Canvas exists and has non-zero physical dimensions after layout.
    const dims = await page.evaluate(() => {
      const canvas = document.querySelector<HTMLCanvasElement>("#ascii-bg");
      return canvas ? { width: canvas.width, height: canvas.height } : null;
    });
    expect(dims, "canvas #ascii-bg must exist").not.toBeNull();
    expect(dims?.width).toBeGreaterThan(0);
    expect(dims?.height).toBeGreaterThan(0);

    // Sample pixel data twice; any rAF tick would shift the bytes.
    const sample = (): Promise<number[]> =>
      page.evaluate(
        ({ w, h }) => {
          const canvas = document.querySelector<HTMLCanvasElement>("#ascii-bg");
          const ctx = canvas?.getContext("2d");
          if (!canvas || !ctx) {
            return [];
          }
          const { data } = ctx.getImageData(0, 0, w, h);
          return [...data];
        },
        { w: CANVAS_SAMPLE_WIDTH, h: CANVAS_SAMPLE_HEIGHT },
      );

    await page.waitForTimeout(100);
    const frameA = await sample();
    await page.waitForTimeout(200);
    const frameB = await sample();
    expect(frameA, "canvas pixels must be stable under reduced motion").toEqual(frameB);
  });

  test("pipeline tablist does not auto-advance", async ({ page }) => {
    await useReducedMotion(page);
    await page.goto("/");

    const tabs = page.getByRole("tab");
    await expect(tabs).toHaveCount(6);
    const firstTab = tabs.first();
    await expect(firstTab).toHaveAttribute("aria-selected", "true");

    // Auto-advance is configured for 5s in default mode, so waiting 4s is
    // long enough to detect it would have fired once if it were running.
    await page.waitForTimeout(4_000);
    await expect(
      firstTab,
      "first tab must stay selected because auto-advance is disabled",
    ).toHaveAttribute("aria-selected", "true");
  });

  test("hero subtext and below-the-fold cards fade in to opacity 1 immediately", async ({
    page,
  }) => {
    await useReducedMotion(page);
    await page.goto("/", { waitUntil: "domcontentloaded" });

    const heroSub = await computedOpacity(page, ".hero-sub");
    expect(heroSub, "hero sub must exist").not.toBeNull();
    expect(heroSub, "hero sub must be fully visible above the fold").toBe("1");

    // [data-fade-in] controls below-the-fold blocks via IntersectionObserver
    // in default mode. Under reduced motion the CSS media query forces
    // opacity 1, so the very first match on the page already sits at 1 with
    // no scroll required.
    const firstFadeIn = await computedOpacity(page, "[data-fade-in]");
    expect(firstFadeIn, "at least one [data-fade-in] element must exist").not.toBeNull();
    expect(firstFadeIn, "[data-fade-in] must be fully visible under reduced motion").toBe("1");
  });
});

test.describe("prefers-reduced-motion: default (animated)", () => {
  test.beforeEach(async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "no-preference" });
  });

  test("pipeline auto-advances past the initial tab after a few seconds", async ({ page }) => {
    await useDefaultMotion(page);
    await page.goto("/");

    const tabs = page.getByRole("tab");
    await expect(tabs.first()).toHaveAttribute("aria-selected", "true");

    // Auto-advance cadence is 5s; allow 12s before concluding selection
    // never moved. Using expect.poll lets the check pass the moment
    // advancement happens instead of waiting the full timeout.
    await expect
      .poll(
        async () => {
          const selections = await tabs.evaluateAll((nodes) =>
            nodes.map((n) => n.getAttribute("aria-selected") === "true"),
          );
          // Report the index of the currently-selected tab; initial is 0.
          return selections.findIndex(Boolean);
        },
        {
          timeout: 12_000,
          message: "pipeline auto-advance must move selection off the first tab",
        },
      )
      .toBeGreaterThan(0);
  });

  test("terminal typer writes progressively (two snapshots 300ms apart differ)", async ({
    page,
  }) => {
    await useDefaultMotion(page);
    await page.goto("/");

    const termLines = page.locator("#term-lines");
    await expect(termLines).toBeVisible();

    // The typer delays 400ms before starting, so wait for it to begin; any
    // non-empty change across the 300ms window proves the animation runs.
    await page.waitForFunction(() => {
      const el = document.querySelector("#term-lines");
      return el !== null && (el.textContent ?? "").length > 0;
    });
    const first = (await termLines.textContent()) ?? "";
    await page.waitForTimeout(300);
    const second = (await termLines.textContent()) ?? "";
    expect(second.length, "terminal must keep typing under default motion").not.toBe(first.length);
  });

  test("below-the-fold fade-in elements reveal on scroll", async ({ page }) => {
    await useDefaultMotion(page);
    await page.goto("/");

    // Pick the features section which is below the fold by design and carries
    // [data-fade-in] on its wrapper. The section has multiple fade-in cards;
    // scope to the first to avoid strict-mode matcher ambiguity.
    const selector = "#how-it-works [data-fade-in]";
    const target = page.locator(selector).first();
    const before = await target.evaluate((el) => getComputedStyle(el).opacity);
    // Can be 0 (initial) or any value < 1; we care it is not already fully opaque.
    expect(Number(before)).toBeLessThan(1);

    await target.scrollIntoViewIfNeeded();
    await expect
      .poll(async () => Number(await target.evaluate((el) => getComputedStyle(el).opacity)), {
        timeout: 3_000,
        message: "fade-in must resolve to opacity 1 after scroll",
      })
      .toBe(1);
  });
});
