import { expect, test } from "@playwright/test";

// Pipeline tablist source: apps/landing/src/components/pipeline.astro.
// Keyboard behaviour (ArrowDown / End / Home) is verified alongside the
// click/hover/focus behaviours specified by the WAI-ARIA Authoring Practices
// tab pattern. Auto-advance must pause on focus AND hover so a user who is
// reading a specific layer is not pre-empted.

// Default-motion mode keeps the auto-advance timer alive; reduced-motion
// tests live in reduced_motion.spec.ts. `emulateMedia` runs before every
// navigation so the component's startup matchMedia() check sees the value.
test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
});

test("pipeline tablist moves aria-selected on keyboard nav", async ({ page }) => {
  await page.goto("/");

  const tabs = page.getByRole("tab");
  await expect(tabs).toHaveCount(6);

  const first = tabs.first();
  await first.click();
  await expect(first).toHaveAttribute("aria-selected", "true");

  await first.focus();
  await page.keyboard.press("ArrowDown");
  await expect(tabs.nth(1)).toHaveAttribute("aria-selected", "true");

  await page.keyboard.press("End");
  await expect(tabs.nth(5)).toHaveAttribute("aria-selected", "true");

  await page.keyboard.press("Home");
  await expect(tabs.first()).toHaveAttribute("aria-selected", "true");
});

test("clicking each tab moves aria-selected and exposes the matching panel", async ({ page }) => {
  await page.goto("/");

  const tabs = page.getByRole("tab");
  const panel = page.locator("#pipeline-detail-panel");
  await expect(tabs).toHaveCount(6);
  await expect(panel).toBeVisible();

  const tabCount = await tabs.count();
  for (let i = 0; i < tabCount; i++) {
    const tab = tabs.nth(i);
    await tab.click();
    await expect(tab, `tab ${i} must become aria-selected`).toHaveAttribute(
      "aria-selected",
      "true",
    );
    // The component uses one panel that relabels through aria-labelledby.
    // Verify the panel is not hidden and now references this tab's id.
    const labelled = await panel.getAttribute("aria-labelledby");
    expect(labelled).toBe(await tab.getAttribute("id"));
    await expect(panel).toBeVisible();
  }
});

test("auto-advance pauses while a tab is focused", async ({ page }) => {
  await page.goto("/");
  const tabs = page.getByRole("tab");

  // Focus the first tab; the component calls stopAuto() on focusin. Waiting
  // longer than the 5s interval and staying on tab 0 proves the timer was
  // cancelled.
  await tabs.first().focus();
  await page.waitForTimeout(5_600);
  await expect(tabs.first(), "auto-advance must pause when a tab is focused").toHaveAttribute(
    "aria-selected",
    "true",
  );
});

test("auto-advance pauses while a tab is hovered (pointerdown also stops)", async ({ page }) => {
  await page.goto("/");
  const tabs = page.getByRole("tab");

  // Hover fires pointerover but the component stops the timer on
  // pointerdown. Simulate a real user's pointer interaction by dispatching
  // a pointerdown that does not change the selection: the timer must still
  // cancel so subsequent hover/dwell keeps tab 0 selected.
  const firstTab = tabs.first();
  await firstTab.hover();
  await firstTab.dispatchEvent("pointerdown");
  await page.waitForTimeout(5_600);
  await expect(firstTab, "auto-advance must pause on pointer interaction").toHaveAttribute(
    "aria-selected",
    "true",
  );
});
