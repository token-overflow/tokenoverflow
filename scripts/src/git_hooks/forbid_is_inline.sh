#!/usr/bin/env bash
#
# Forbids `is:inline` in `.astro` files (landing app).
#
# Reason: no tool enforces this yet (ESLint/Astro/oxlint/CSP/astro check).
#
# Requirement: all scripts must be same-origin external files to satisfy
# CSP `script-src 'self'` (no hashes/nonces). Fails if any scanned file
# contains `is:inline`.

set -euo pipefail

if [[ $# -eq 0 ]]; then
  exit 0
fi

# Reject `is:inline`. Intentionally narrow: matches the directive with or
# without a surrounding whitespace run so an attribute inside an attribute
# value (rare) does not slip through.
if grep --line-number --with-filename --extended-regexp 'is:inline' "$@"; then
  echo "error: 'is:inline' is prohibited on the landing app." >&2
  exit 1
fi

exit 0
