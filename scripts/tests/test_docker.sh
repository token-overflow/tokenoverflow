#!/usr/bin/env bash
# Tests for docker.sh

set_up_before_script() {
  # shellcheck source=scripts/src/docker.sh
  source "scripts/src/docker.sh"
}

set_up() {
  TOKENOVERFLOW_MOCK_LOG=""

  docker() { TOKENOVERFLOW_MOCK_LOG+="docker $*;"; }
  export -f docker
  curl() { TOKENOVERFLOW_MOCK_LOG+="curl $*;"; }
  export -f curl
}

tear_down() {
  unset TOKENOVERFLOW_MOCK_LOG
}

# --- redeploy_local ---

function test_redeploy_local_calls_compose_down() {
  redeploy_local
  assert_contains "docker compose down -v" "$TOKENOVERFLOW_MOCK_LOG"
}

function test_redeploy_local_calls_compose_up() {
  redeploy_local
  assert_contains "docker compose up -d" "$TOKENOVERFLOW_MOCK_LOG"
}

function test_redeploy_local_calls_health_check() {
  redeploy_local
  assert_contains "curl http://localhost:8080/health" "$TOKENOVERFLOW_MOCK_LOG"
}

function test_redeploy_local_executes_in_correct_order() {
  redeploy_local

  # Verify order: down before up before curl
  local down_pos up_pos curl_pos
  down_pos=$(echo "$TOKENOVERFLOW_MOCK_LOG" | grep -bo "compose down" | head -1 | cut -d: -f1)
  up_pos=$(echo "$TOKENOVERFLOW_MOCK_LOG" | grep -bo "compose up" | head -1 | cut -d: -f1)
  curl_pos=$(echo "$TOKENOVERFLOW_MOCK_LOG" | grep -bo "curl" | head -1 | cut -d: -f1)

  assert_equals "true" "$([ "$down_pos" -lt "$up_pos" ] && echo true || echo false)"
  assert_equals "true" "$([ "$up_pos" -lt "$curl_pos" ] && echo true || echo false)"
}
