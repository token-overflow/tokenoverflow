import { expect, test } from "@playwright/test";
import { config } from "@tokenoverflow/config";

// The www -> apex 301 is a CloudFront Function at the viewer-request event,
// not application code. Testing it requires hitting the live CloudFront
// edge, which cannot be emulated cheaply with the local SWS (SWS does not
// route by Host header). Skipped under `TOKENOVERFLOW_ENV=local`; runs when
// the selector points at a live edge (`production`).
test.describe("www -> apex redirect (live only)", () => {
  test.skip(config.env === "local", "TOKENOVERFLOW_ENV=local; skipping live redirect check.");

  test("www.tokenoverflow.io/ returns 301 to apex", async ({ request }) => {
    const target = config.landing.base_url.replace(/\/$/, "");
    const response = await request.get(`${target}/`, { maxRedirects: 0 });
    expect(response.status()).toBe(301);
    expect(response.headers()["location"]).toMatch(/^https:\/\/tokenoverflow\.io\/?$/);
  });
});
