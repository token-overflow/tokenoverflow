import { vi } from "vitest";

/// Stub `@tanstack/solid-router` so route files can be imported in
/// vitest without booting the SolidJS SSR runtime. The stub records the
/// options object on the returned `Route` so integration tests can pull
/// the handler back out via `Route.options.server.handlers.GET`.
vi.mock("@tanstack/solid-router", () => ({
  createFileRoute: () => (options: unknown) => ({ options }),
  createRootRoute: (options: unknown) => ({ options }),
}));
