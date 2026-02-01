#!/usr/bin/env bash
# Tests for act.sh

set_up_before_script() {
  # shellcheck source=scripts/src/act.sh
  source "scripts/src/act.sh"
}

set_up() {
  TOKENOVERFLOW_ACT_ARGS=""

  act() { TOKENOVERFLOW_ACT_ARGS="$*"; }
  export -f act
}

tear_down() {
  unset TOKENOVERFLOW_ACT_ARGS
}

# --- act_terraform ---

function test_act_terraform_defaults_event_to_push() {
  act_terraform
  assert_contains "push" "$TOKENOVERFLOW_ACT_ARGS"
}

function test_act_terraform_uses_custom_event() {
  act_terraform pull_request
  assert_contains "pull_request" "$TOKENOVERFLOW_ACT_ARGS"
}

function test_act_terraform_uses_terraform_workflow() {
  act_terraform
  assert_contains "-W .github/workflows/terraform.yml" "$TOKENOVERFLOW_ACT_ARGS"
}

function test_act_terraform_uses_correct_event_file() {
  act_terraform
  assert_contains "-e .github/act/event_push_main.json" "$TOKENOVERFLOW_ACT_ARGS"
}

function test_act_terraform_event_file_matches_event() {
  act_terraform pull_request
  assert_contains "-e .github/act/event_pull_request_main.json" "$TOKENOVERFLOW_ACT_ARGS"
}

function test_act_terraform_includes_secret_file() {
  act_terraform
  assert_contains "--secret-file .act.secrets" "$TOKENOVERFLOW_ACT_ARGS"
}

# --- act_deploy ---

function test_act_deploy_defaults_event_to_push() {
  act_deploy
  assert_contains "push" "$TOKENOVERFLOW_ACT_ARGS"
}

function test_act_deploy_uses_deploy_workflow() {
  act_deploy
  assert_contains "-W .github/workflows/deploy_api.yml" "$TOKENOVERFLOW_ACT_ARGS"
}

function test_act_deploy_uses_custom_event() {
  act_deploy workflow_dispatch
  assert_contains "workflow_dispatch" "$TOKENOVERFLOW_ACT_ARGS"
}
