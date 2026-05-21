import { HeadContent, Outlet, Scripts, createRootRoute } from "@tanstack/solid-router";

/// Root layout for the BFF. The BFF serves no UI today (only OAuth and a
/// liveness probe), but `__root.tsx` is required so the route tree compiles.
export const Route = createRootRoute({
  component: RootComponent,
});

function RootComponent() {
  return (
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <HeadContent />
      </head>
      <body>
        <Outlet />
        <Scripts />
      </body>
    </html>
  );
}
