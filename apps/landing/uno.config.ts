import presetAttributify from "@unocss/preset-attributify";
import presetWind4 from "@unocss/preset-wind4";
import transformerDirectives from "@unocss/transformer-directives";
import transformerVariantGroup from "@unocss/transformer-variant-group";
import { tokenOverflowPreset } from "@tokenoverflow/unocss-config";
import { defineConfig, presetIcons } from "unocss";

/**
 * UnoCSS config for the landing page.
 *
 * Presets:
 *   - preset-wind4:        Tailwind-compatible utilities (colors, spacing, typography).
 *   - preset-icons:        On-demand icon classes (i-ph-* and i-simple-icons-*).
 *   - preset-attributify:  Attribute-style utilities (bg="orange-500" text="white").
 *                          Use on elements with 5+ utility classes for readability.
 *   - tokenOverflowPreset:  Brand font stacks layered on top of Wind4.
 *
 * Transformers:
 *   - transformer-variant-group: hover:(bg-orange-600 text-white) shorthand.
 *   - transformer-directives:    @apply, @screen, and theme('colors.orange.500')
 *                                inside <style> blocks. Required wherever a Wind4
 *                                token must flow into raw CSS.
 */
export default defineConfig({
  presets: [
    presetWind4(),
    presetIcons({
      collections: {
        ph: async () => {
          const { default: icons } = await import("@iconify-json/ph/icons.json");
          return icons;
        },
        simple: async () => {
          const { default: icons } = await import("@iconify-json/simple-icons/icons.json");
          return icons;
        },
      },
    }),
    presetAttributify(),
    tokenOverflowPreset(),
  ],
  transformers: [transformerVariantGroup(), transformerDirectives()],
  // Block Wind4 utilities that collide with shared primitives authored in
  // global.css:
  //  - `focus-ring`: Wind4 parses it as the `focus:ring` variant+utility pair
  //    and emits an extra 1px currentColor box-shadow on top of the
  //    intended orange outline.
  //  - `container`: Wind4's `.container` utility emits responsive `max-width`
  //    inside `@media` blocks whose cascade order overrides the custom
  //    `.container { max-width: 80rem }` below the xl breakpoint, clamping
  //    every section to 40-64rem on smaller desktops.
  blocklist: [/^focus-ring$/, /^container$/],
});
