import { createRouter } from "@tanstack/solid-router";

import { routeTree } from "./routeTree.gen";

/// Build the TanStack router for the BFF.
///
/// `routeTree` is auto-generated from `src/routes/**` by the start plugin
/// during build/dev.
export function getRouter() {
  return createRouter({
    routeTree,
    defaultPreload: "intent",
    scrollRestoration: true,
  });
}
