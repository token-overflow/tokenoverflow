# @tokenoverflow/unocss-config

Thin UnoCSS preset that extends `preset-wind4` with the TokenOverflow brand
font stacks from `@tokenoverflow/design-tokens`. Consumers call
`tokenOverflowPreset()` after `presetWind4()` in `uno.config.ts`.

## `dist/` Layout

The package compiles to `dist/index.{js,d.ts}` and exposes both entrypoints
through the `exports` field. Compiled output is required to sidestep a Bun
isolated-linker issue with TypeScript workspace packages that publish raw
`.ts` sources:
[alchemy-run/alchemy#994](https://github.com/alchemy-run/alchemy/issues/994).
