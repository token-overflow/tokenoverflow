import { createFileRoute } from "@tanstack/solid-router";

export const Route = createFileRoute("/health")({
  server: {
    handlers: {
      GET: () => Response.json({ ok: true }),
    },
  },
});
