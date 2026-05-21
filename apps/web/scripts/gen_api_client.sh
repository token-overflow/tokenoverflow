#!/usr/bin/env bash
set -euo pipefail

# Generate the OpenAPI spec from the Rust API binary, then emit a typed
# TypeScript SDK into src/utils/api/_generated/. The whole directory is
# treated as generated output: hey-api wipes it on every run, and the
# `*.gen.ts` glob in `.gitignore` keeps the tree out of version control.
# The hand-written wrapper lives at src/utils/api/waitlist.server.ts and
# imports `addToWaitlist` from `_generated/sdk.gen`.

cd "$(dirname "$0")/../../.."

SPEC_FILE="$(mktemp -t openapi-spec.XXXXXX.json)"
trap 'rm -f "$SPEC_FILE"' EXIT

# Set `TOKENOVERFLOW_BUNDLED_LIBS=1` on runners without system libpq / OpenSSL to statically bundle them from C source.
CARGO_ARGS=(--quiet --release --manifest-path apps/api/Cargo.toml)
if [[ "${TOKENOVERFLOW_BUNDLED_LIBS:-}" == "1" ]]; then
  CARGO_ARGS+=(--features bundled-libs)
fi

cargo run "${CARGO_ARGS[@]}" -- --openapi-json > "$SPEC_FILE"

cd apps/web
bun x @hey-api/openapi-ts \
  --input "$SPEC_FILE" \
  --output src/utils/api/_generated

# hey-api emits TypeScript that does not satisfy our `strict` +
# `exactOptionalPropertyTypes` tsconfig settings. Prepending
# `// @ts-nocheck` keeps the workspace typecheck strict for first-party
# code while letting us consume the SDK without bending the project
# settings. The generated files are gitignored, so the marker is
# regenerated on every codegen run.
find src/utils/api/_generated -type f -name '*.ts' -exec \
  sh -c 'printf "// @ts-nocheck\n%s" "$(cat "$1")" > "$1"' shell {} \;
