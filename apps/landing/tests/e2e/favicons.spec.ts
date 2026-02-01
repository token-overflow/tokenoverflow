import { expect, test } from "@playwright/test";

// Favicon/manifest/apple-touch-icon markup lives in
// apps/landing/src/layouts/base.astro. Each asset is copied through from
// apps/landing/public/ during build; this spec verifies both the link tags
// and that each referenced file is actually reachable.

interface FaviconProbe {
  readonly selector: string;
  readonly expectedHref: string;
  readonly expectedContentType: RegExp;
}

const PROBES: readonly FaviconProbe[] = [
  {
    selector: 'link[rel="icon"][type="image/svg+xml"]',
    expectedHref: "/favicon.svg",
    expectedContentType: /^image\/svg\+xml/,
  },
  {
    selector: 'link[rel="icon"][type="image/png"][sizes="32x32"]',
    expectedHref: "/favicon-32x32.png",
    expectedContentType: /^image\/png/,
  },
  {
    selector: 'link[rel="apple-touch-icon"]',
    expectedHref: "/apple-touch-icon.png",
    expectedContentType: /^image\/png/,
  },
  {
    selector: 'link[rel="manifest"]',
    expectedHref: "/manifest.webmanifest",
    expectedContentType: /manifest|json/,
  },
];

test("favicon, apple-touch-icon, and manifest tags reference the expected URLs", async ({
  page,
}) => {
  await page.goto("/");

  for (const probe of PROBES) {
    const link = page.locator(probe.selector);
    await expect(link, `selector ${probe.selector} must match exactly one tag`).toHaveCount(1);
    const href = await link.getAttribute("href");
    expect(href, `${probe.selector} href`).toBe(probe.expectedHref);
  }
});

test("each favicon/manifest asset returns 200 with the right Content-Type", async ({ request }) => {
  for (const probe of PROBES) {
    const response = await request.head(probe.expectedHref);
    expect(response.status(), `HEAD ${probe.expectedHref}`).toBe(200);
    const contentType = response.headers()["content-type"] ?? "";
    expect(contentType, `HEAD ${probe.expectedHref} content-type`).toMatch(
      probe.expectedContentType,
    );
  }
});
