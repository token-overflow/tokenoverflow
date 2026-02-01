import sitemap from "@astrojs/sitemap";
import { defineConfig } from "astro/config";
import autoprefixer from "autoprefixer";
import UnoCSS from "unocss/astro";

export default defineConfig({
  site: "https://tokenoverflow.io",
  output: "static",
  // `inlineStylesheets: "never"` keeps CSP `style-src 'self'` working without
  // requiring `'unsafe-inline'` or style-hash allowlists: every stylesheet
  // ships as an external /_astro/*.css file served same-origin.
  //
  // No Astro Fonts API: typography uses a soft-prefer-Inter, fall-through-
  // to-system-fonts stack defined in @tokenoverflow/design-tokens. Removing
  // the Fonts API removes the only inline <style> block on the page, so CSP
  // can stay strictly `style-src 'self'`. Trade-off: visitors without Inter
  // installed see SF Pro / Segoe UI / Roboto, which are visually close.
  build: { format: "directory", inlineStylesheets: "never" },
  integrations: [sitemap(), UnoCSS({ injectReset: true })],
  vite: {
    logLevel: "warn",
    clearScreen: false,
    build: {
      // Disables Vite's automatic inlining so every bundled <script> block emits
      // as an external /_astro/*.js file. The design requires this so CSP script-src
      // 'self' holds without hashes or nonces.
      assetsInlineLimit: 0,
    },
    css: {
      postcss: {
        // Autoprefixer runs in the Vite PostCSS pipeline so authored CSS stays
        // prefix-free. Targets come from the `browserslist` entry in package.json.
        // This lets Stylelint's property-no-vendor-prefix rule stay enabled with no
        // manual prefixes checked into src/.
        plugins: [autoprefixer()],
      },
    },
  },
});
