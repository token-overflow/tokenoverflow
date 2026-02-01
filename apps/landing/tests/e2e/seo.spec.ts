import { expect, test } from "@playwright/test";

import { findNode, readJsonLd } from "../common/seo_utils";

// SEO-surface assertions for GET /. Everything here drives against what a
// social crawler or search engine would see in the raw HTML. The component
// source is at apps/landing/src/components/seo.astro plus the per-page data
// in apps/landing/src/pages/index.astro.

const EXPECTED_TITLE = "TokenOverflow: Stack Overflow for AI Coding Agents";
const CANONICAL = "https://tokenoverflow.io/";
const GITHUB_URL = "https://github.com/token-overflow/tokenoverflow";
const X_URL = "https://x.com/tokenoverflowio";

test("title and meta description are present with sane lengths", async ({ page }) => {
  await page.goto("/");

  await expect(page).toHaveTitle(EXPECTED_TITLE);

  const description = await page.locator('meta[name="description"]').getAttribute("content");
  expect(description, "meta description must be set").not.toBeNull();
  expect(description ?? "").not.toBe("");
  // 70..320 is the soft SERP range: shorter gets truncated, longer is Google-pruned.
  const len = (description ?? "").length;
  expect(len, `description length ${len} must be between 70 and 320`).toBeGreaterThanOrEqual(70);
  expect(len, `description length ${len} must be between 70 and 320`).toBeLessThanOrEqual(320);
});

test("canonical link points at the homepage root", async ({ page }) => {
  await page.goto("/");
  const href = await page.locator('link[rel="canonical"]').getAttribute("href");
  expect(href).toBe(CANONICAL);
});

test("open-graph tags are present and populated", async ({ page }) => {
  await page.goto("/");

  const expectedOg: Record<string, (value: string | null) => void> = {
    "og:type": (value) => expect(value).toBe("website"),
    "og:url": (value) => expect(value).toBe(CANONICAL),
    "og:site_name": (value) => expect(value).toBe("TokenOverflow"),
    "og:title": (value) => expect(value).toBe(EXPECTED_TITLE),
    "og:description": (value) => {
      expect(value).not.toBeNull();
      expect((value ?? "").length).toBeGreaterThan(0);
    },
    "og:image": (value) => {
      expect(value).not.toBeNull();
      expect((value ?? "").length).toBeGreaterThan(0);
    },
    "og:image:width": (value) => expect(value).toBe("1200"),
    "og:image:height": (value) => expect(value).toBe("630"),
    "og:locale": (value) => expect(value).toBe("en_US"),
  };

  for (const [property, assertion] of Object.entries(expectedOg)) {
    const value = await page.locator(`meta[property="${property}"]`).getAttribute("content");
    assertion(value);
  }
});

test("twitter card tags are present with summary_large_image", async ({ page }) => {
  await page.goto("/");

  await expect(page.locator('meta[name="twitter:card"]')).toHaveAttribute(
    "content",
    "summary_large_image",
  );
  const title = await page.locator('meta[name="twitter:title"]').getAttribute("content");
  expect(title).toBe(EXPECTED_TITLE);
  const desc = await page.locator('meta[name="twitter:description"]').getAttribute("content");
  expect(desc ?? "").not.toBe("");
  const image = await page.locator('meta[name="twitter:image"]').getAttribute("content");
  expect(image ?? "").not.toBe("");
});

test("exactly one JSON-LD script is present and parses as valid JSON", async ({ page }) => {
  await page.goto("/");
  const scripts = page.locator('script[type="application/ld+json"]');
  await expect(scripts).toHaveCount(1);
  const text = (await scripts.first().textContent()) ?? "";
  expect(() => JSON.parse(text)).not.toThrow();
});

test("JSON-LD graph has Organization + WebSite only, SoftwareApplication removed", async ({
  page,
}) => {
  await page.goto("/");
  const graph = await readJsonLd(page);

  expect(graph["@context"]).toBe("https://schema.org");
  expect(Array.isArray(graph["@graph"])).toBe(true);

  const members = graph["@graph"] ?? [];
  expect(members, "graph must have exactly two members").toHaveLength(2);

  const org = findNode(graph, "Organization");
  const site = findNode(graph, "WebSite");
  expect(org, "Organization node must exist").toBeDefined();
  expect(site, "WebSite node must exist").toBeDefined();

  const softwareApp = findNode(graph, "SoftwareApplication");
  expect(
    softwareApp,
    "SoftwareApplication node was removed earlier and must stay removed",
  ).toBeUndefined();
});

test("Organization node has the expected @id and publisher linkage", async ({ page }) => {
  await page.goto("/");
  const graph = await readJsonLd(page);

  const org = findNode(graph, "Organization");
  const site = findNode(graph, "WebSite");
  expect(org).toBeDefined();
  expect(site).toBeDefined();

  const orgId = String(org?.["@id"] ?? "");
  expect(
    orgId.endsWith("#organization"),
    `Organization @id "${orgId}" must end with #organization`,
  ).toBe(true);

  const publisher = site?.["publisher"] as Record<string, unknown> | undefined;
  expect(publisher?.["@id"]).toBe(orgId);
});

test("Organization.sameAs lists the GitHub and X profiles", async ({ page }) => {
  await page.goto("/");
  const graph = await readJsonLd(page);
  const org = findNode(graph, "Organization");
  const sameAs = org?.["sameAs"];
  expect(Array.isArray(sameAs)).toBe(true);
  expect(sameAs).toEqual(expect.arrayContaining([GITHUB_URL, X_URL]));
});
