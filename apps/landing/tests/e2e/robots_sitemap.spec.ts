import { expect, test } from "@playwright/test";

// Robots and sitemap are plain-text/XML endpoints; Playwright's `request`
// fixture is the idiomatic fit because we only need to check response shape,
// not render anything. The sitemap-index lives at /sitemap-index.xml and is
// emitted by @astrojs/sitemap.

const SITEMAP_INDEX_URL = "https://tokenoverflow.io/sitemap-index.xml";
const HOMEPAGE_URL = "https://tokenoverflow.io/";

test("GET /robots.txt returns a plain-text User-agent and Sitemap line", async ({ request }) => {
  const response = await request.get("/robots.txt");
  expect(response.status()).toBe(200);
  expect(response.headers()["content-type"] ?? "").toMatch(/^text\/plain/);

  const body = await response.text();
  // Allow optional leading whitespace before the first directive; the crawler
  // spec is tolerant, but catching an empty/missing User-agent line matters.
  expect(body.trimStart().startsWith("User-agent: *")).toBe(true);
  expect(body).toContain(`Sitemap: ${SITEMAP_INDEX_URL}`);
});

test("GET /sitemap-index.xml returns 200 and valid XML referencing the homepage", async ({
  request,
}) => {
  const indexResponse = await request.get("/sitemap-index.xml");
  expect(indexResponse.status()).toBe(200);
  const indexBody = await indexResponse.text();

  // Lightweight XML shape checks. A full XML parser would be overkill and
  // would drag in a dependency; string assertions cover the invariants.
  expect(indexBody.startsWith("<?xml")).toBe(true);
  expect(indexBody).toMatch(/<sitemapindex[\s\S]*<\/sitemapindex>/);

  // The index links to a child sitemap; fetch that and assert the homepage
  // URL appears in at least one <loc>.
  const childMatch = indexBody.match(/<loc>([^<]+)<\/loc>/);
  expect(childMatch, "sitemap-index.xml must reference at least one child sitemap").not.toBeNull();
  const childUrl = childMatch?.[1] ?? "";
  const childPath = new URL(childUrl).pathname;
  const childResponse = await request.get(childPath);
  expect(childResponse.status()).toBe(200);
  const childBody = await childResponse.text();
  expect(childBody).toContain(`<loc>${HOMEPAGE_URL}</loc>`);
});
