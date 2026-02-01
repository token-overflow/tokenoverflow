# Landing Page Guidelines

## Authoring Rules

Utility-first is mandatory. Every styling decision lives as a utility class on
markup; a scoped `<style>` block exists only for patterns the utility grammar
cannot express.

### When scoped `<style>` is allowed

If a rule does not fit one of these, it belongs on markup:

1. **`@keyframes` bodies.** UnoCSS cannot emit keyframe percentage stops. Hook
   the animation via a custom `animate-<name>` utility in UnoCSS theme so
   markup reads `animate-shimmer`, or reference the keyframe by name from
   inside the same scoped block.
2. **`::before` / `::after` with generated `content:`.** Blueprint-hex
   overlays, spine decorations, arrow glyphs between cards, traffic-light
   dot chrome. Use `@apply` inside the rule to apply utility-like tokens
   (`content: ''; @apply absolute inset-0 pointer-events-none;`).
3. **Complex cascade selectors.** `:has(...)`, `:nth-child(n)`, sibling
   combinators (`+`, `~`), descendant-state like
   `.parent:hover .descendant`, and `:global(...)` overrides that reach
   into inline SVG subtrees. These are structurally beyond utility class
   scope.
4. **`@font-face`.** Emitted by Astro Fonts API; never hand-authored here.
5. **Base / reset rules.** Only in `global.css`: `:root`, `body`, `html`,
   `::selection`, global attribute selectors (`[data-fade-in]`).
6. **Third-party library overrides** dictated by the library. None today.

### When to reach for `@apply`

Use it only when:

- The target is a pseudo-element (`::before`, `::after`) whose `content:`
  generates it from outside markup, so it has no class to carry utilities.
- The selector is a state-driven cascade (`:focus-visible`, `:nth-child(n)`,
  `:has(...)`) that can never be expressed as a self-class utility.
- A rule cannot name utilities without sacrificing readability, and the
  declaration body is only one or two lines.

For cross-component shared primitives (`.container`, `.section*`) prefer
authoring raw CSS with `theme()` literals: every site uses one class
reference, and `@apply` would emit both the primitive's body AND each
underlying utility.

### Variant groups, attributify, icons

- **Variant groups**: `hover:(bg-orange-600 text-white focus-ring)` collapses
  repeated `hover:` prefixes. Always prefer over writing each variant by hand.
- **Attributify**: available via `preset-attributify` but applied only where
  an element has 5+ utility classes AND using attributes genuinely improves
  scan-ability. Short class strings stay as class strings. Do not force it.
- **Icons**: prefer `<span class="i-ph-arrow-right text-lg" />` (via
  `preset-icons`) over inline SVG for anything that is not hand-animated.

### When to use `theme()`

Inside authored CSS blocks: `theme('colors.orange.500')` instead of `#f97316`.
The `@unocss/transformer-directives` transformer rewrites the call into the
resolved Wind4 color at build time. Raw hex is forbidden in authored CSS
outside of data-URL payloads (where the string is embedded in an SVG).

### Forbidden patterns

- `is:inline` on `<script>`. Every script ships as a bundled `<script>` block
  so Astro emits it as a same-origin `.js` file. `forbid-is-inline` pre-commit
  hook enforces.
- `style=` attribute on markup for anything except unavoidable system-UI
  references (macOS traffic-light hex dots, Claude Code's salmon accent).
  Every such case carries an adjacent comment explaining why.
- `@media (prefers-reduced-motion: reduce)` for a per-element override;
  reach for `motion-reduce:*` instead.
- `@media (width <= 48rem)` for responsive layout; reach for `lt-md:*` or
  `max-md:*` variants.
- `stylelint-disable`, `@ts-ignore`, test skips. Fix issues, don't suppress.
- Raw hex literals in authored source (`rg '#[0-9a-fA-F]{3,6}\b' src/`
  returns zero outside data-URL grids and the documented system-UI exceptions).

## Theme & Design Tokens

1. **Exact Wind4 match** - use the Wind4 utilities. Never keep an arbitrary
   utility whose value already matches a Wind4 scale point.
2. **Within small tolerance** - snap to the nearest Wind4 token.
3. **Recurring unique value** (3+ occurrences that do not snap) - promote
   to a brand token in `@tokenoverflow/design-tokens`, exposed as a utility
   class through `@tokenoverflow/unocss-config`. Use the utility in markup
   and `@apply <token>` in authored CSS blocks so the primitive value lives
   in one place.
4. **Genuine one-off unique** - keep as an arbitrary utility
   (`text-[clamp(...)]`, `max-w-[50rem]`) in markup or a CSS literal in a
   `<style>` block, with a one-line comment explaining why it does not snap
   or promote.

## Image Handling

Raster images referenced from `src/**/*.astro` MUST go through Astro's
`<Image>` or `<Picture>`. This gives us Sharp-compressed AVIF/WebP output,
responsive srcset, and CLS-safe dimensions for free. Raw `<img src="photo.png">`
in components is forbidden and the `forbid-raw-raster-img` pre-commit hook
enforces it.

SVG is exempt: inline `<svg>` or `<img src="foo.svg">` is fine.

`public/` is reserved for assets that need stable, unhashed URLs:
favicons, the OG image, the manifest. Those paths are hardcoded in
browsers and crawlers and must not flow through `<Image>` hashing.
