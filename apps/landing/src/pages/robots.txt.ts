import type { APIRoute } from "astro";

export const GET: APIRoute = () =>
  new Response(
    [
      "User-agent: *",
      "Allow: /",
      "",
      "Sitemap: https://tokenoverflow.io/sitemap-index.xml",
      "",
    ].join("\n"),
    { headers: { "content-type": "text/plain; charset=utf-8" } },
  );
