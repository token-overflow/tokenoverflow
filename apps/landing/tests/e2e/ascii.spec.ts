import { expect, test } from "@playwright/test";

// ASCII canvas source: apps/landing/src/components/ascii.astro. Runs a rAF
// loop under default motion, renders a single static frame under reduced
// motion (see reduced_motion.spec.ts for the static-frame assertion).

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
});

test("<canvas id='ascii-bg'> exists with non-zero width/height after layout", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("#ascii-bg")).toBeAttached();
  const dims = await page.evaluate(() => {
    const canvas = document.querySelector<HTMLCanvasElement>("#ascii-bg");
    return canvas ? { width: canvas.width, height: canvas.height } : null;
  });
  expect(dims).not.toBeNull();
  expect(dims?.width, "canvas width must be non-zero after layout").toBeGreaterThan(0);
  expect(dims?.height, "canvas height must be non-zero after layout").toBeGreaterThan(0);
});

test("animation is running: two frames 300ms apart differ", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("#ascii-bg")).toBeAttached();

  // Sample the full canvas width but only a thin horizontal strip near the
  // top of the hero (y=40). The rAF loop redraws 15% of rows per 33ms tick,
  // so most sampled rows change within 300ms. expect.poll guards against
  // worst-case scheduling where a sample snapshots between ticks.
  //
  // The digest folds all four channels (R, G, B, A) with a positional
  // multiplier so a glyph shift shows up even when the sampled row is
  // currently rendering all-black (accent=0) text: R=G=B=0 pixels still
  // contribute positional weight through the alpha channel, which tracks
  // which columns have painted glyphs vs. transparent background.
  const sample = (): Promise<string> =>
    page.evaluate(() => {
      const canvas = document.querySelector<HTMLCanvasElement>("#ascii-bg");
      const ctx = canvas?.getContext("2d");
      if (!canvas || !ctx) {
        return "";
      }
      const { data } = ctx.getImageData(0, 40, canvas.width, 40);
      // Return a hash-like string to make equality cheap and to avoid
      // paying for a 150k-entry array serialisation across the CDP bridge.
      // Math.imul keeps the multiplication inside Int32, preventing the
      // recurrence from overflowing to Infinity on wide canvases (51k+ pixels
      // sampled here would cross Number.MAX_VALUE in ~50 iterations under
      // plain `*`).
      let digest = 0;
      for (let i = 0; i < data.length; i += 4) {
        digest = Math.imul(digest, 31) + data[i]! + data[i + 1]! + data[i + 2]! + data[i + 3]!;
      }
      return digest.toString();
    });

  // Astro ships the canvas script as an external ES module, so on first
  // paint the canvas element exists before any draw call has landed. WebKit
  // schedules that module later than Chromium/Firefox, so without this wait
  // the baseline sample can latch onto the pre-draw all-zero state (digest
  // "0") and the 2s poll below races the first real draw. Poll until a
  // non-"0" digest appears, then lock that as frameA.
  let frameA = "0";
  await expect
    .poll(
      async () => {
        frameA = await sample();
        return frameA;
      },
      {
        timeout: 5_000,
        message: "ASCII canvas must have produced at least one drawn frame",
      },
    )
    .not.toBe("0");
  expect(frameA.length, "canvas must produce pixel data").toBeGreaterThan(0);
  // Allow up to 2 seconds; 300ms is the happy path but a busy CI VM may
  // drop frames and defer the first rAF tick well past that mark.
  await expect
    .poll(async () => await sample(), {
      timeout: 2_000,
      message: "ASCII canvas must animate: pixel digest must change",
    })
    .not.toBe(frameA);
});
