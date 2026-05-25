#!/usr/bin/env bash
# Wraps `trivy fs` to auto-skip paths from `.gitignore`.
# Avoids maintaining a separate skip list (Trivy lacks native support).
# Uses `git ls-files` to get ignored entries, splits into dirs and files.

set -euo pipefail

ENTRIES=$(git ls-files --others --ignored --exclude-standard --directory)

SKIP_DIRS=$(awk '/\/$/ { sub(/\/$/, ""); print }' <<<"$ENTRIES" | paste -sd ',' -)
SKIP_FILES=$(awk '!/\/$/' <<<"$ENTRIES" | paste -sd ',' -)

ARGS=(fs --disable-telemetry --exit-code 1 --severity='HIGH,CRITICAL')
[ -n "$SKIP_DIRS" ] && ARGS+=(--skip-dirs "$SKIP_DIRS")
[ -n "$SKIP_FILES" ] && ARGS+=(--skip-files "$SKIP_FILES")

exec trivy "${ARGS[@]}" .
