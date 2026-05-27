#!/usr/bin/env bash
# Runs coverage for unit and integration tiers across all workspace members.
# E2E coverage is enforced in CI via e2e_test.yml.
# Requires: cargo-llvm-cov (installed via setup_cargo_tools)
# Requires: Docker running for testcontainers (integration tests)

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
    --lib --test unit --test integration \
    --fail-under-lines "${REQUIRED_COVERAGE}"
