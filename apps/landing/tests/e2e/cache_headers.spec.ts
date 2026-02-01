import { expect, test } from "@playwright/test";

// Cache-Control contract mirrors apps/landing/static-web-server.toml and the
// production Cloudflare origin. HTML revalidates on every pageview; hashed
// /_astro/** assets get a year-long immutable cache. Both are asserted
// through HEAD so no bodies transfer.

const HTML_CACHE = "public, max-age=0, must-revalidate";
const IMMUTABLE_CACHE = "public, max-age=31536000, immutable";

test("HEAD / emits the HTML revalidate Cache-Control", async ({ request }) => {
  const response = await request.head("/");
  expect(response.status()).toBe(200);
  expect(response.headers()["cache-control"]).toBe(HTML_CACHE);
});

test("HEAD on an /_astro/ asset emits the immutable one-year Cache-Control", async ({
  request,
}) => {
  // Fetch the homepage HTML, parse out the first /_astro/ URL (CSS or JS),
  // and HEAD that concrete asset. Picking the asset from the real page
  // guarantees we test a file with a live hash.
  const pageResponse = await request.get("/");
  const html = await pageResponse.text();
  const match = html.match(/\/_astro\/[A-Za-z0-9_.@-]+\.(?:css|js)/);
  expect(match, "index.html must reference at least one /_astro/ asset").not.toBeNull();
  const assetPath = match?.[0] ?? "";

  const assetResponse = await request.head(assetPath);
  expect(assetResponse.status()).toBe(200);
  expect(assetResponse.headers()["cache-control"]).toBe(IMMUTABLE_CACHE);
});
