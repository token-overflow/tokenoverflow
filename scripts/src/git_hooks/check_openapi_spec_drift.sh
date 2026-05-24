#!/usr/bin/env bash
# Fails when apps/api/openapi.json is out of sync with the utoipa
# annotations in apps/api/src/. Distinguishes "cargo build broken" from
# "spec drift" so a failing build is not mis-reported as a drift error.

# Drop -e: both gen_api_spec and diff are inspected with `if !` and would
# abort the script before we can branch on the failure.
set -uo pipefail

# shellcheck source=scripts/src/api.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/api.sh"

EXPECTED="apps/api/openapi.json"
ACTUAL="$(mktemp -t openapi-spec.XXXXXX.json)"
CARGO_LOG="$(mktemp -t openapi-spec-cargo.XXXXXX.log)"
trap 'rm -f "$ACTUAL" "$CARGO_LOG"' EXIT

if ! gen_api_spec "$ACTUAL" 2>"$CARGO_LOG"; then
  echo "ERROR: cargo failed while regenerating ${EXPECTED}." >&2
  echo "Fix the API build, then re-commit." >&2
  cat "$CARGO_LOG" >&2
  exit 1
fi

if ! diff -q "$EXPECTED" "$ACTUAL" > /dev/null; then
  echo "ERROR: ${EXPECTED} is stale (out of sync with apps/api/ source)." >&2
  echo "Regenerate and re-commit:" >&2
  echo "  source scripts/src/includes.sh && gen_api_spec" >&2
  echo "  git add ${EXPECTED}" >&2
  diff -u "$EXPECTED" "$ACTUAL" | head -80 >&2
  exit 1
fi
