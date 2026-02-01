// Shared SEO/JSON-LD helpers. Extracted because two specs parse the JSON-LD
// payload: the SEO spec itself and tests that reason about the payload's
// Organization/WebSite relationship. Keeping the logic in one place means a
// schema change only has to be reflected once.
import type { Page } from "@playwright/test";

export interface JsonLdGraph {
  readonly "@context"?: unknown;
  readonly "@graph"?: readonly Record<string, unknown>[];
}

/**
 * Returns the single JSON-LD script's parsed payload. Throws if the page has
 * zero or multiple such scripts, or if the content fails to parse, so callers
 * never need to handle those edge cases inline.
 */
export const readJsonLd = async (page: Page): Promise<JsonLdGraph> => {
  const scripts = page.locator('script[type="application/ld+json"]');
  const count = await scripts.count();
  if (count !== 1) {
    throw new Error(`expected exactly one JSON-LD script, found ${count}`);
  }
  const raw = await scripts.first().textContent();
  if (raw === null) {
    throw new Error("JSON-LD script has no text content");
  }
  return JSON.parse(raw) as JsonLdGraph;
};

/**
 * Pulls a single `@graph` member by `@type`. Returns undefined if absent so
 * the caller chooses the failure message. Case-sensitive comparison matches
 * schema.org convention.
 */
export const findNode = (graph: JsonLdGraph, type: string): Record<string, unknown> | undefined => {
  if (!Array.isArray(graph["@graph"])) {
    return undefined;
  }
  return graph["@graph"].find((node) => node["@type"] === type);
};
