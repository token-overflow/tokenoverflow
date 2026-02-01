#!/usr/bin/env bash
# Tests for setup.sh

set_up_before_script() {
  # shellcheck source=scripts/src/utils.sh
  source "scripts/src/utils.sh"
  # shellcheck source=scripts/src/setup.sh
  source "scripts/src/setup.sh"
}

set_up() {
  TEST_DIR=$(mktemp -d)
  TOKENOVERFLOW_ORIGINAL_HOME="$HOME"
  export HOME="$TEST_DIR"
  TOKENOVERFLOW_MOCK_LOG=""

  # Mock all external installers
  brew() { TOKENOVERFLOW_MOCK_LOG+="brew $*;"; }
  export -f brew
  curl() { echo "mock-curl"; }
  export -f curl
  rustup-init() { TOKENOVERFLOW_MOCK_LOG+="rustup-init $*;"; }
  export -f rustup-init
  rustup() { TOKENOVERFLOW_MOCK_LOG+="rustup $*;"; }
  export -f rustup
  cargo() { TOKENOVERFLOW_MOCK_LOG+="cargo $*;"; }
  export -f cargo
  prek() { TOKENOVERFLOW_MOCK_LOG+="prek $*;"; }
  export -f prek
}

tear_down() {
  export HOME="$TOKENOVERFLOW_ORIGINAL_HOME"
  rm -rf "$TEST_DIR"
  unset TOKENOVERFLOW_MOCK_LOG TOKENOVERFLOW_ORIGINAL_HOME
}

# --- setup_env ---

function test_setup_env_adds_postgres_to_path() {
  # Remove postgres from PATH if present
  PATH=$(echo "$PATH" | tr ':' '\n' | grep -v "postgresql@18" | tr '\n' ':')
  setup_env >/dev/null 2>&1
  assert_contains "/opt/homebrew/opt/postgresql@18/bin" "$PATH"
}

function test_setup_env_is_idempotent() {
  PATH=$(echo "$PATH" | tr ':' '\n' | grep -v "postgresql@18" | tr '\n' ':')
  setup_env >/dev/null 2>&1
  setup_env >/dev/null 2>&1

  local count
  count=$(echo "$PATH" | tr ':' '\n' | grep -c "postgresql@18")
  assert_equals "1" "$count"
}

# --- setup_aws ---

function test_setup_aws_creates_config_file() {
  mkdir -p "$TEST_DIR/.aws"
  setup_aws >/dev/null 2>&1
  assert_file_exists "$TEST_DIR/.aws/config"
}

function test_setup_aws_contains_sso_session() {
  mkdir -p "$TEST_DIR/.aws"
  setup_aws >/dev/null 2>&1
  assert_file_contains "$TEST_DIR/.aws/config" "sso-session tokenoverflow"
}

function test_setup_aws_contains_dev_profile() {
  mkdir -p "$TEST_DIR/.aws"
  setup_aws >/dev/null 2>&1
  assert_file_contains "$TEST_DIR/.aws/config" "profile tokenoverflow-dev-admin"
}

function test_setup_aws_contains_prod_profile() {
  mkdir -p "$TEST_DIR/.aws"
  setup_aws >/dev/null 2>&1
  assert_file_contains "$TEST_DIR/.aws/config" "profile tokenoverflow-prod-admin"
}

function test_setup_aws_contains_root_profile() {
  mkdir -p "$TEST_DIR/.aws"
  setup_aws >/dev/null 2>&1
  assert_file_contains "$TEST_DIR/.aws/config" "profile tokenoverflow-root-admin"
}

function test_setup_aws_contains_correct_account_ids() {
  mkdir -p "$TEST_DIR/.aws"
  setup_aws >/dev/null 2>&1
  assert_file_contains "$TEST_DIR/.aws/config" "871610744185"
  assert_file_contains "$TEST_DIR/.aws/config" "591120835062"
  assert_file_contains "$TEST_DIR/.aws/config" "058170691494"
}

function test_setup_aws_is_idempotent() {
  mkdir -p "$TEST_DIR/.aws"
  setup_aws >/dev/null 2>&1
  setup_aws >/dev/null 2>&1

  local count
  count=$(grep -c "# Tokenoverflow START" "$TEST_DIR/.aws/config")
  assert_equals "1" "$count"
}

# --- setup_brew ---

function test_setup_brew_calls_brew_bundle() {
  setup_brew >/dev/null 2>&1
  assert_contains "brew bundle install --file=Brewfile" "$TOKENOVERFLOW_MOCK_LOG"
}

# --- setup_rust ---

function test_setup_rust_calls_rustup_init() {
  setup_rust >/dev/null 2>&1
  assert_contains "rustup-init -y --no-modify-path" "$TOKENOVERFLOW_MOCK_LOG"
}

function test_setup_rust_installs_nightly() {
  setup_rust >/dev/null 2>&1
  assert_contains "rustup toolchain install nightly" "$TOKENOVERFLOW_MOCK_LOG"
}

# --- setup_precommit ---

function test_setup_precommit_calls_prek_install() {
  setup_precommit >/dev/null 2>&1
  assert_contains "prek install" "$TOKENOVERFLOW_MOCK_LOG"
}
