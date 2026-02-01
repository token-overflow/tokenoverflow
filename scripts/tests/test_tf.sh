#!/usr/bin/env bash
# Tests for tf.sh

set_up_before_script() {
  # shellcheck source=scripts/src/tf.sh
  source "scripts/src/tf.sh"
}

set_up() {
  TEST_DIR=$(mktemp -d)
  cd "$TEST_DIR" || exit 1

  TOKENOVERFLOW_MOCK_LOG=""

  # Create directory structure expected by tg()
  mkdir -p "infra/terraform/live"/{global,dev,prod}

  # Create .opentofu-version for setup_opentofu()
  echo "1.11.5" >.opentofu-version

  # Mock tofuenv
  tofuenv() { TOKENOVERFLOW_MOCK_LOG+="tofuenv $*;"; }
  export -f tofuenv

  # Mock aws (default: session valid)
  aws() {
    TOKENOVERFLOW_MOCK_LOG+="aws $*;"
    return 0
  }
  export -f aws

  # Mock terragrunt
  terragrunt() { TOKENOVERFLOW_MOCK_LOG+="terragrunt $*;"; }
  export -f terragrunt
}

tear_down() {
  cd /
  rm -rf "$TEST_DIR"
  unset TOKENOVERFLOW_MOCK_LOG AWS_PROFILE TOFUENV_TOFU_VERSION
}

# --- setup_opentofu ---

function test_setup_opentofu_exports_version() {
  setup_opentofu
  assert_equals "1.11.5" "$TOFUENV_TOFU_VERSION"
}

function test_setup_opentofu_calls_tofuenv_install() {
  setup_opentofu
  assert_contains "tofuenv install" "$TOKENOVERFLOW_MOCK_LOG"
}

function test_setup_opentofu_calls_tofuenv_use_with_version() {
  setup_opentofu
  assert_contains "tofuenv use 1.11.5" "$TOKENOVERFLOW_MOCK_LOG"
}

# --- tg: argument validation ---

function test_tg_missing_args_returns_error() {
  local output
  output=$(tg 2>&1)
  assert_exit_code "1"
  assert_contains "Usage:" "$output"
}

function test_tg_missing_env_returns_error() {
  local output
  output=$(tg plan 2>&1)
  assert_exit_code "1"
  assert_contains "Usage:" "$output"
}

function test_tg_invalid_env_returns_error() {
  local output
  output=$(tg plan staging 2>&1)
  assert_exit_code "1"
  assert_contains "Unknown environment" "$output"
}

# --- tg: environment-to-profile mapping ---

function test_tg_global_maps_to_root_profile() {
  tg plan global >/dev/null 2>&1
  assert_equals "tokenoverflow-root-admin" "$AWS_PROFILE"
}

function test_tg_dev_maps_to_dev_profile() {
  tg plan dev >/dev/null 2>&1
  assert_equals "tokenoverflow-dev-admin" "$AWS_PROFILE"
}

function test_tg_prod_maps_to_prod_profile() {
  tg plan prod >/dev/null 2>&1
  assert_equals "tokenoverflow-prod-admin" "$AWS_PROFILE"
}

# --- tg: AWS session handling ---

function test_tg_skips_login_when_session_valid() {
  tg plan dev >/dev/null 2>&1
  assert_not_contains "aws sso login" "$TOKENOVERFLOW_MOCK_LOG"
}

function test_tg_calls_sso_login_when_session_expired() {
  # Override aws mock to fail on sts check
  aws() {
    TOKENOVERFLOW_MOCK_LOG+="aws $*;"
    if [[ "$1" == "sts" ]]; then return 1; fi
    return 0
  }
  export -f aws

  tg plan dev >/dev/null 2>&1
  assert_contains "aws sso login --profile tokenoverflow-dev-admin" "$TOKENOVERFLOW_MOCK_LOG"
}

# --- tg: terragrunt invocation ---

function test_tg_calls_terragrunt_with_correct_args() {
  tg plan dev >/dev/null 2>&1
  assert_contains "terragrunt run --all plan" "$TOKENOVERFLOW_MOCK_LOG"
}

function test_tg_apply_passes_apply_action() {
  tg apply prod >/dev/null 2>&1
  assert_contains "terragrunt run --all apply" "$TOKENOVERFLOW_MOCK_LOG"
}
