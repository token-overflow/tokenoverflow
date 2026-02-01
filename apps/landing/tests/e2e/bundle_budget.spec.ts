import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

import { expect, test } from "@playwright/test";

// Bundle-budget assertions. Written as a Playwright spec instead of a
// standalone Bun script because the rest of the e2e tier already runs after
// `bun run build`, so dist/ is guaranteed fresh here; a pre-commit-only
// script would mean two separate build triggers.
//
// Approach:
//   1. Read dist/index.html from disk and locate the <script type="module"
//      src="/_astro/<component>.astro_astro_type_script_..."> entries. Astro
//      names each bundle after the Astro component that owns the <script>,
//      which gives us a stable component-to-asset mapping without shelling
//      out to any Astro-internal API.
//   2. Read each matched JS file, gzip its bytes, and assert the compressed
//      size is within budget.

interface Budget {
  readonly component: string;
  readonly maxBytes: number;
  readonly description: string;
}

// All budgets are gzipped-bytes; Cloudflare negotiates gzip or brotli with
// every client, so gzip matches the wire weight the browser sees in the
// median case. The exact brotli size is usually ~5% smaller, so gzip is a
// conservative upper bound.
const BUDGETS: readonly Budget[] = [
  { component: "ascii", maxBytes: 3 * 1_024, description: "ASCII canvas" },
  { component: "terminal", maxBytes: Math.floor(1.5 * 1_024), description: "Terminal typer" },
  { component: "pipeline", maxBytes: 2 * 1_024, description: "Pipeline tablist" },
];

// Fade-in observer and smooth-scroll are two inline <script> blocks inside
// base.astro, so Astro emits them as base.astro_astro_type_script_index_0
// and ..._index_1 .js. They share a single 1 KB budget per the design.
const BASE_COMBINED_BUDGET = 1_024;

const DIST_DIR = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "dist");
const SCRIPT_SRC_RE =
  /<script[^>]*type="module"[^>]*src="(\/_astro\/([A-Za-z0-9@_.-]+?)\.astro_astro_type_script_index_\d+_lang\.[A-Za-z0-9]+\.js)"/g;

interface ScriptMatch {
  readonly component: string;
  readonly path: string;
}

const readAllScriptRefs = async (): Promise<readonly ScriptMatch[]> => {
  const html = await readFile(join(DIST_DIR, "index.html"), "utf8");
  const matches: ScriptMatch[] = [];
  for (const match of html.matchAll(SCRIPT_SRC_RE)) {
    matches.push({ component: match[2] ?? "", path: match[1] ?? "" });
  }
  return matches;
};

const gzippedSize = async (distRelPath: string): Promise<number> => {
  const absolute = join(DIST_DIR, distRelPath);
  const bytes = await readFile(absolute);
  return gzipSync(bytes).length;
};

test.describe("JS bundle budgets (gzipped)", () => {
  for (const budget of BUDGETS) {
    test(`${budget.description} stays under ${budget.maxBytes} bytes gzipped`, async () => {
      const refs = await readAllScriptRefs();
      const match = refs.find((r) => r.component === budget.component);
      expect(
        match,
        `dist/index.html must reference ${budget.component}.astro script`,
      ).toBeDefined();
      const size = await gzippedSize(match!.path);
      expect(
        size,
        `${budget.component}.astro bundle was ${size} bytes gzipped; budget ${budget.maxBytes}`,
      ).toBeLessThanOrEqual(budget.maxBytes);
    });
  }

  test(`base.astro fade-in + smooth-scroll scripts together stay under ${BASE_COMBINED_BUDGET} bytes gzipped`, async () => {
    const refs = await readAllScriptRefs();
    const baseScripts = refs.filter((r) => r.component === "base");
    expect(
      baseScripts.length,
      "dist/index.html must reference at least one base.astro script",
    ).toBeGreaterThan(0);

    let combined = 0;
    for (const ref of baseScripts) {
      combined += await gzippedSize(ref.path);
    }
    expect(
      combined,
      `base.astro bundles totalled ${combined} bytes gzipped; budget ${BASE_COMBINED_BUDGET}`,
    ).toBeLessThanOrEqual(BASE_COMBINED_BUDGET);
  });
});
