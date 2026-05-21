import { tanstackStart } from "@tanstack/solid-start/plugin/vite";
import { nitro } from "nitro/vite";
import { defineConfig } from "vite";
import viteSolid from "vite-plugin-solid";

// Local docker exports `NITRO_PRESET=node-server` to build a Node runtime
// image. CI omits the env var so Vite defaults to the AWS Lambda zip preset.
const preset = process.env["NITRO_PRESET"] ?? "aws-lambda";

// Pinned to a Nitro 3.0 beta on purpose. The previous stable `nitro@3.0.0`
// shipped a `treeshake.moduleSideEffects` allowlist in its Vite plugin that
// excluded the `#nitro-vite-setup` virtual module, so Rolldown dropped the
// side-effect import that installs the SSR `globalThis.fetch` override. The
// resulting build emitted `.output/server/index.mjs` with zero route
// registrations, and every request hung in `fetchHandler` because the
// renderer's internal `fetch(req, { viteEnv: "ssr" })` fell through to a
// real network call back to the listening port.
//
// Tracked at https://github.com/nitrojs/nitro/issues/4085, fixed by
// https://github.com/nitrojs/nitro/pull/4164, first shipped in
// `v3.0.260415-beta`. Drop the beta pin once a non-beta v3.0.x ships.
export default defineConfig({
  server: { port: 3_000 },
  plugins: [
    tanstackStart(),
    // The `plugins` array is the list of Nitro runtime plugins to
    // register. `log_context.ts` wires the per-invocation
    // pino-lambda request id into the logger so every line carries
    // `request_id`.
    nitro({ preset, plugins: ["./src/server/plugins/log_context.ts"] }),
    viteSolid({ ssr: true }),
  ],
});
