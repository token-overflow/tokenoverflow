import { type Preset, definePreset } from "unocss";
import { brandSizes, fontStack } from "@tokenoverflow/design-tokens";

const basePreset = definePreset(() => ({
  name: "@tokenoverflow/unocss-config",
  theme: {
    // Wind4 reads `theme.font.*` (not `theme.fontFamily.*`) for
    // `--font-*` vars. Using the wrong key falls back to system fonts.
    //
    // Both stacks soft-prefer the brand face (Inter / JetBrains Mono) if the
    // user has it installed locally, then fall through to system fonts. No
    // webfont is shipped: keeps CSP `style-src 'self'` strict and saves a
    // network round-trip on first paint.
    font: {
      sans: fontStack.sans,
      mono: fontStack.mono,
    },
  },
  // Brand typography utilities that sit outside the Wind4 `text-*` scale.
  rules: [
    ["text-label", { "font-size": brandSizes.label }],
    ["text-micro", { "font-size": brandSizes.micro }],
  ],
}));

/**
 * Thin UnoCSS preset that extends preset-wind4 with Token Overflow brand font
 * stacks and typography utilities.
 */
export const tokenOverflowPreset = (): Preset => basePreset() as Preset;
