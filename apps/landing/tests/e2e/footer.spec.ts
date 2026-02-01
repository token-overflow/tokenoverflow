import { expect, test } from "@playwright/test";

// Footer has three moving parts: the dynamic copyright year (rendered at
// build time from new Date().getFullYear()), and the two outbound social
// links. Assertions target concrete selectors rather than role matches
// because the footer text otherwise collides with other `contentinfo`
// children in the DOM tree.

const GITHUB_URL = "https://github.com/token-overflow/tokenoverflow";
const X_URL = "https://x.com/tokenoverflowio";

test("footer renders the current year in the copyright text", async ({ page }) => {
  await page.goto("/");
  const footer = page.locator("footer");
  const year = new Date().getFullYear();
  // Be permissive with the "All rights reserved." tail; we only need to
  // confirm the year/brand presence and catch stale years.
  await expect(footer).toContainText(new RegExp(`\\u00a9\\s*${year}\\s*TokenOverflow`));
});

test("footer GitHub link targets the public repo and opens in a new tab", async ({ page }) => {
  await page.goto("/");
  const githubLink = page.locator("footer").getByRole("link", { name: /github/i });
  await expect(githubLink).toHaveAttribute("href", GITHUB_URL);
  await expect(githubLink).toHaveAttribute("target", "_blank");
  // Rel may be "noopener" or "noopener noreferrer"; we only enforce the
  // security-critical "noopener" token so future additions (noreferrer)
  // do not break the test.
  const rel = (await githubLink.getAttribute("rel")) ?? "";
  expect(rel.split(/\s+/)).toContain("noopener");
  // Aria-label is required to give screen-reader users a name; axe catches
  // the absence globally but we surface it here as a diagnostic failure.
  await expect(githubLink).toHaveAttribute("aria-label", /github/i);
});

test("footer X link targets the public profile and opens in a new tab", async ({ page }) => {
  await page.goto("/");
  const xLink = page.locator("footer").getByRole("link", { name: /x profile/i });
  await expect(xLink).toHaveAttribute("href", X_URL);
  await expect(xLink).toHaveAttribute("target", "_blank");
  const rel = (await xLink.getAttribute("rel")) ?? "";
  expect(rel.split(/\s+/)).toContain("noopener");
  await expect(xLink).toHaveAttribute("aria-label", /x profile/i);
});
