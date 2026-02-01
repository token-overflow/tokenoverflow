import { expect, test } from "@playwright/test";

// Terminal source: apps/landing/src/components/terminal.astro.
// The script types a fake Claude Code session with a fixed list of steps.
// Under default motion, the full script takes ~10 seconds to finish before
// the loop restarts; we poll for distinctive tokens rather than fix a
// timeout so the test stays robust under load.

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
});

test("terminal element is present on /", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".terminal")).toBeVisible();
  await expect(page.locator("#term-lines")).toBeAttached();
});

test("terminal types a fake Claude Code session that includes the scripted steps", async ({
  page,
}) => {
  await page.goto("/");

  const termLines = page.locator("#term-lines");
  // The script writes progressively. `pip install` appears well after
  // `ModuleNotFoundError`, so polling for the final distinctive step avoids
  // a race where we read between pause boundaries and miss a later line.
  // 30s covers the full happy path (type + all pauses + verify) with
  // headroom for slow CI.
  await expect
    .poll(async () => (await termLines.textContent()) ?? "", { timeout: 30_000 })
    .toMatch(/pip install pydantic/);

  const text = (await termLines.textContent()) ?? "";
  // Grep for distinctive commands/steps without pinning exact spacing.
  expect(text).toMatch(/pydantic/);
  expect(text).toMatch(/ModuleNotFoundError/);
  expect(text).toMatch(/tokenoverflow/i);
  expect(text).toMatch(/Found verified solution/);
});

test("terminal output uses textContent only (no unexpected inner HTML tags)", async ({ page }) => {
  await page.goto("/");
  const termLines = page.locator("#term-lines");

  // Wait for at least one scripted line to appear, then inspect.
  await expect
    .poll(async () => {
      const t = (await termLines.textContent()) ?? "";
      return t.length;
    })
    .toBeGreaterThan(0);

  // The only elements inside #term-lines should be div.tl line wrappers and
  // optional inline spans (for the typed-prompt prefix/typed halves). The
  // script appends text via textContent, so no <script>, <img>, <iframe>,
  // or other tags must appear.
  const disallowed = await page.evaluate(() => {
    const root = document.querySelector("#term-lines");
    if (!root) {
      return [] as string[];
    }
    const all = root.querySelectorAll("*");
    const allowed = new Set(["DIV", "SPAN"]);
    return [...all].map((el) => el.tagName).filter((t) => !allowed.has(t));
  });
  expect(disallowed, "terminal must only contain text nodes wrapped in div/span").toEqual([]);
});
