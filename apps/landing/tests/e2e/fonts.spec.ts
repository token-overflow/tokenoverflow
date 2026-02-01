import { expect, test } from "@playwright/test";

// The landing ships no webfonts. Typography uses a soft-prefer-Inter,
// fall-through-to-system-fonts stack defined in @tokenoverflow/design-tokens.
// These tests pin that contract: no <link rel="preload" as="font">, no
// @font-face declarations, and the rendered font-family chain starts with
// "Inter" so a locally-installed copy is honored.

test("no font preloads are emitted", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator('link[rel="preload"][as="font"]')).toHaveCount(0);
});

test("no @font-face rules ship in any inline or external stylesheet", async ({ page }) => {
  await page.goto("/");
  const hasFontFace = await page.evaluate(() => {
    const sheets = [...document.styleSheets] as CSSStyleSheet[];
    const safeRules = (sheet: CSSStyleSheet): CSSRule[] => {
      try {
        return [...sheet.cssRules];
      } catch {
        return [];
      }
    };
    return sheets.some((sheet) => safeRules(sheet).some((r) => r instanceof CSSFontFaceRule));
  });
  expect(hasFontFace).toBe(false);
});

test("body computed font-family soft-prefers Inter", async ({ page }) => {
  await page.goto("/");
  const fontFamily = await page.evaluate(() => getComputedStyle(document.body).fontFamily);
  expect(fontFamily).toMatch(/^['"]?Inter['"]?,/);
});
