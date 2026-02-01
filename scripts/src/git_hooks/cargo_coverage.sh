#!/usr/bin/env bash
# Runs coverage for all workspace members across all test tiers.
# Requires: cargo-llvm-cov (installed via setup_cargo_tools)
# Requires: Docker running for testcontainers (integration tests)
# Requires: Docker Compose services running for e2e tests (docker compose up -d --build api)

set -euo pipefail

# shellcheck source=/dev/null
[[ -f "${HOME}/.cargo/env" ]] && source "${HOME}/.cargo/env"

REQUIRED_COVERAGE=95

if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "Error: cargo-llvm-cov is not installed."
    echo "Run: cargo install cargo-llvm-cov"
    exit 1
fi

cargo +nightly llvm-cov \
    --workspace \
    --lib --test unit --test integration --test e2e \
    --fail-under-lines "${REQUIRED_COVERAGE}"
