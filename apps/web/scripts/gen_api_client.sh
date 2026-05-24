#!/usr/bin/env bash
set -euo pipefail

# Read the committed OpenAPI spec at apps/api/openapi.json and emit a typed
# TypeScript SDK into src/utils/api/_generated/. The spec itself is produced
# by the Rust API binary (`--openapi-json`) and refreshed by the
# `check-openapi-spec-drift` pre-commit hook, so no cargo is needed here.
# The whole _generated/ directory is treated as generated output: hey-api
# wipes it on every run, and the `*.gen.ts` glob in `.gitignore` keeps the
# tree out of version control. The hand-written wrapper lives at
# src/utils/api/waitlist.server.ts and imports `addToWaitlist` from
# `_generated/sdk.gen`. The hey-api version is governed by the workspace
# devDependency pin in apps/web/package.json; no --version flag needed.

cd "$(dirname "$0")/../../.."

SPEC_FILE="apps/api/openapi.json"
if [[ ! -f "$SPEC_FILE" ]]; then
  echo "Missing ${SPEC_FILE}. Regenerate it:" >&2
  echo "  source scripts/src/includes.sh && gen_api_spec" >&2
  exit 1
fi

cd apps/web
bun x @hey-api/openapi-ts \
  --input "../../${SPEC_FILE}" \
  --output src/utils/api/_generated

# hey-api emits TypeScript that does not satisfy our `strict` +
# `exactOptionalPropertyTypes` tsconfig settings. Prepending
# `// @ts-nocheck` keeps the workspace typecheck strict for first-party
# code while letting us consume the SDK without bending the project
# settings. The generated files are gitignored, so the marker is
# regenerated on every codegen run.
find src/utils/api/_generated -type f -name '*.ts' -exec \
  sh -c 'printf "// @ts-nocheck\n%s" "$(cat "$1")" > "$1"' shell {} \;
