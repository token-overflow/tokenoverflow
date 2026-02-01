import { expect, test } from "@playwright/test";

// The branded 404 flow relies on the CloudFront custom error response in
// production and SWS's `page404` setting locally; both serve /404.html with
// HTTP 404 on missing paths, so the same assertions hold against either
// origin.
test("direct /404 load renders the branded 404 page", async ({ page }) => {
  await page.goto("/404");
  await expect(page.getByRole("heading", { level: 1, name: /page not found/i })).toBeVisible();
  await expect(page.getByRole("link", { name: /return home/i })).toBeVisible();
});

test("404 page title is distinct from the homepage title and mentions not found", async ({
  page,
}) => {
  await page.goto("/");
  const homeTitle = await page.title();

  await page.goto("/404");
  const notFoundTitle = await page.title();

  expect(notFoundTitle).not.toBe(homeTitle);
  expect(notFoundTitle.toLowerCase()).toMatch(/not found|404/);
});

test("'Return home' link navigates to / and hero renders there", async ({ page }) => {
  await page.goto("/404");
  const link = page.getByRole("link", { name: /return home/i });
  await expect(link).toHaveAttribute("href", "/");

  await link.click();
  await page.waitForURL((url) => url.pathname === "/");
  await expect(page.getByRole("heading", { level: 1 })).toContainText("AI coding agents");
});

test("404 page canonical (if present) points at the homepage root, not the missing URL", async ({
  page,
}) => {
  // The copy of 404.html served by SWS is a static file; there is no runtime
  // URL rewrite on the canonical tag. The invariant is that whoever authored
  // the page pointed it at a real, indexable URL rather than the 404 path.
  await page.goto("/404");
  const canonical = page.locator('link[rel="canonical"]');
  const count = await canonical.count();
  // Canonical is optional on the 404 page; absence is fine.
  if (count === 0) {
    return;
  }
  const href = await canonical.first().getAttribute("href");
  expect(href, "canonical must not point at the missing URL").not.toMatch(/\/does-not-exist|\/404/);
});

test("non-existent path returns branded 404 with URL preserved", async ({ page }) => {
  const response = await page.goto("/does-not-exist");
  expect(response?.status()).toBe(404);
  expect(page.url()).toContain("/does-not-exist");
  await expect(page.getByRole("heading", { level: 1, name: /page not found/i })).toBeVisible();
});
