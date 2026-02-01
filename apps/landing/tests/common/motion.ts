// Motion-preference helpers. Playwright's emulateMedia() toggles the CSS
// media query, but any component that reads the value via matchMedia() at
// startup only sees it if we emulate before navigation. Centralising the
// call order here avoids subtle flakes where a page is loaded first and
// the preference is applied too late to affect the initial render.
import type { Page } from "@playwright/test";

export const useReducedMotion = async (page: Page): Promise<void> => {
  await page.emulateMedia({ reducedMotion: "reduce" });
};

export const useDefaultMotion = async (page: Page): Promise<void> => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
};

/**
 * Evaluates getComputedStyle(selector).opacity in the page context. Returns
 * null when the selector matches no element so the caller's assertion
 * message can describe the selector instead of failing on `Cannot read
 * property 'opacity' of null`.
 */
export const computedOpacity = (page: Page, selector: string): Promise<string | null> =>
  page.evaluate((sel) => {
    const el = document.querySelector(sel);
    if (!(el instanceof HTMLElement)) {
      return null;
    }
    return getComputedStyle(el).opacity;
  }, selector);
