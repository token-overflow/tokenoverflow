#!/usr/bin/env bash

# Regenerate the API's OpenAPI spec from utoipa annotations.
# Writes to apps/api/openapi.json by default. The pre-commit drift hook
# passes a temp path to compare against the committed copy.
function gen_api_spec() {
  local out="${1:-apps/api/openapi.json}"
  cargo run \
    --quiet --release \
    --manifest-path apps/api/Cargo.toml \
    -- --openapi-json > "$out"
}
