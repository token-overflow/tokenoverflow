#!/usr/bin/env bash
#
# Forbids raw `<img>` in `apps/landing/src/**/*.astro`.
#
# Use `<Image>`/`<Picture>` (optimized, responsive, avoids CLS).
# Raw `<img>` bypasses optimization and ships unoptimized assets.
#
# No existing rule (astro check / oxlint / eslint-plugin-astro).
# Enforced via grep pre-commit hook.
#
# Scope: `.astro` files in `apps/landing/src` only.
# Excludes: `public/`, SVGs, and `data:` URLs.
#
# Hook scans files itself (argv unused) for consistency.

set -euo pipefail

src_dir="apps/landing/src"

if [[ ! -d "$src_dir" ]]; then
  # Nothing to scan (e.g., running from outside the repo or pre-landing).
  exit 0
fi

# Match `<img ... src="<path>.(png|jpe?g|webp|avif)">`, single or double
# quotes, with arbitrary attributes between `<img` and `src`. The `[^"']*`
# inside the quote captures everything but the closing quote so it works for
# both quote styles in a single pattern. `data:` URLs are excluded by the
# inverse match below.
pattern='<img[^>]*src[[:space:]]*=[[:space:]]*("[^"]*\.(png|jpe?g|webp|avif)"|'"'"'[^'"'"']*\.(png|jpe?g|webp|avif)'"'"')'

violations=$(
  grep -r -l -E "$pattern" "$src_dir" --include='*.astro' 2>/dev/null || true
)

if [[ -n "$violations" ]]; then
  {
    echo "error: raw raster <img> tags in apps/landing/src/**/*.astro are forbidden."
    echo "Route raster images through Astro's <Image> or <Picture> component so"
    echo "Sharp emits responsive AVIF/WebP with explicit dimensions."
    echo "Offending files:"
    echo "$violations"
  } >&2
  exit 1
fi

exit 0
