#!/usr/bin/env bash
# Tests for dev_token.sh

SCRIPT="scripts/src/dev_token.sh"

# -- help / argument parsing --

function test_help_flag_prints_usage() {
  local output
  output=$($SCRIPT --help 2>&1)
  assert_exit_code "0"
  assert_contains "Usage:" "$output"
  assert_contains "--sub" "$output"
  assert_contains "--expiry" "$output"
}

function test_unknown_option_returns_error() {
  local output
  output=$($SCRIPT --bogus 2>&1)
  assert_exit_code "1"
  assert_contains "Unknown option" "$output"
}

# -- token format --

function test_default_invocation_produces_three_part_jwt() {
  local token
  token=$($SCRIPT)
  assert_exit_code "0"

  local dot_count
  dot_count=$(echo "$token" | tr -cd '.' | wc -c | tr -d ' ')
  assert_equals "2" "$dot_count"
}

function test_token_header_contains_rs256() {
  local token
  token=$($SCRIPT)

  local header
  header=$(echo "$token" | cut -d. -f1 | base64 -d 2>/dev/null || true)
  # base64url decoding: add padding and translate chars
  header=$(echo "$token" | cut -d. -f1 | tr '_-' '/+' | base64 -d 2>/dev/null)

  assert_contains '"alg":"RS256"' "$(echo "$header" | tr -d ' ')"
  assert_contains '"kid":"test-key-1"' "$(echo "$header" | tr -d ' ')"
}

# -- custom subject --

function test_custom_sub_appears_in_payload() {
  local token
  token=$($SCRIPT --sub user_custom_42)

  local payload
  payload=$(echo "$token" | cut -d. -f2 | tr '_-' '/+')
  # Add padding
  local pad=$(( 4 - ${#payload} % 4 ))
  [[ $pad -lt 4 ]] && payload="${payload}$(printf '=%.0s' $(seq 1 "$pad"))"
  payload=$(echo "$payload" | base64 -d 2>/dev/null)

  assert_contains '"sub":"user_custom_42"' "$(echo "$payload" | tr -d ' ')"
}

# -- expiry parsing --

function test_expiry_seconds() {
  local token
  token=$($SCRIPT --expiry 30s)
  assert_exit_code "0"

  local payload
  payload=$(echo "$token" | cut -d. -f2 | tr '_-' '/+')
  local pad=$(( 4 - ${#payload} % 4 ))
  [[ $pad -lt 4 ]] && payload="${payload}$(printf '=%.0s' $(seq 1 "$pad"))"
  payload=$(echo "$payload" | base64 -d 2>/dev/null)

  local iat exp
  iat=$(echo "$payload" | jq -r '.iat')
  exp=$(echo "$payload" | jq -r '.exp')
  local diff=$((exp - iat))
  assert_equals "30" "$diff"
}

function test_expiry_minutes() {
  local token
  token=$($SCRIPT --expiry 5m)
  assert_exit_code "0"

  local payload
  payload=$(echo "$token" | cut -d. -f2 | tr '_-' '/+')
  local pad=$(( 4 - ${#payload} % 4 ))
  [[ $pad -lt 4 ]] && payload="${payload}$(printf '=%.0s' $(seq 1 "$pad"))"
  payload=$(echo "$payload" | base64 -d 2>/dev/null)

  local iat exp
  iat=$(echo "$payload" | jq -r '.iat')
  exp=$(echo "$payload" | jq -r '.exp')
  local diff=$((exp - iat))
  assert_equals "300" "$diff"
}

function test_expiry_hours() {
  local token
  token=$($SCRIPT --expiry 2h)
  assert_exit_code "0"

  local payload
  payload=$(echo "$token" | cut -d. -f2 | tr '_-' '/+')
  local pad=$(( 4 - ${#payload} % 4 ))
  [[ $pad -lt 4 ]] && payload="${payload}$(printf '=%.0s' $(seq 1 "$pad"))"
  payload=$(echo "$payload" | base64 -d 2>/dev/null)

  local iat exp
  iat=$(echo "$payload" | jq -r '.iat')
  exp=$(echo "$payload" | jq -r '.exp')
  local diff=$((exp - iat))
  assert_equals "7200" "$diff"
}

function test_expiry_days() {
  local token
  token=$($SCRIPT --expiry 3d)
  assert_exit_code "0"

  local payload
  payload=$(echo "$token" | cut -d. -f2 | tr '_-' '/+')
  local pad=$(( 4 - ${#payload} % 4 ))
  [[ $pad -lt 4 ]] && payload="${payload}$(printf '=%.0s' $(seq 1 "$pad"))"
  payload=$(echo "$payload" | base64 -d 2>/dev/null)

  local iat exp
  iat=$(echo "$payload" | jq -r '.iat')
  exp=$(echo "$payload" | jq -r '.exp')
  local diff=$((exp - iat))
  assert_equals "259200" "$diff"
}

function test_expiry_years() {
  local token
  token=$($SCRIPT --expiry 1y)
  assert_exit_code "0"

  local payload
  payload=$(echo "$token" | cut -d. -f2 | tr '_-' '/+')
  local pad=$(( 4 - ${#payload} % 4 ))
  [[ $pad -lt 4 ]] && payload="${payload}$(printf '=%.0s' $(seq 1 "$pad"))"
  payload=$(echo "$payload" | base64 -d 2>/dev/null)

  local iat exp
  iat=$(echo "$payload" | jq -r '.iat')
  exp=$(echo "$payload" | jq -r '.exp')
  local diff=$((exp - iat))
  assert_equals "31536000" "$diff"
}

function test_default_expiry_is_one_hour() {
  local token
  token=$($SCRIPT)

  local payload
  payload=$(echo "$token" | cut -d. -f2 | tr '_-' '/+')
  local pad=$(( 4 - ${#payload} % 4 ))
  [[ $pad -lt 4 ]] && payload="${payload}$(printf '=%.0s' $(seq 1 "$pad"))"
  payload=$(echo "$payload" | base64 -d 2>/dev/null)

  local iat exp
  iat=$(echo "$payload" | jq -r '.iat')
  exp=$(echo "$payload" | jq -r '.exp')
  local diff=$((exp - iat))
  assert_equals "3600" "$diff"
}

# -- claims from config --

function test_issuer_matches_local_config() {
  local token
  token=$($SCRIPT)

  local payload
  payload=$(echo "$token" | cut -d. -f2 | tr '_-' '/+')
  local pad=$(( 4 - ${#payload} % 4 ))
  [[ $pad -lt 4 ]] && payload="${payload}$(printf '=%.0s' $(seq 1 "$pad"))"
  payload=$(echo "$payload" | base64 -d 2>/dev/null)

  local iss
  iss=$(echo "$payload" | jq -r '.iss')
  assert_equals "tokenoverflow-test" "$iss"
}

function test_audience_matches_local_config() {
  local token
  token=$($SCRIPT)

  local payload
  payload=$(echo "$token" | cut -d. -f2 | tr '_-' '/+')
  local pad=$(( 4 - ${#payload} % 4 ))
  [[ $pad -lt 4 ]] && payload="${payload}$(printf '=%.0s' $(seq 1 "$pad"))"
  payload=$(echo "$payload" | base64 -d 2>/dev/null)

  local aud
  aud=$(echo "$payload" | jq -r '.aud')
  assert_equals "http://localhost:8080" "$aud"
}

function test_default_sub_is_system() {
  local token
  token=$($SCRIPT)

  local payload
  payload=$(echo "$token" | cut -d. -f2 | tr '_-' '/+')
  local pad=$(( 4 - ${#payload} % 4 ))
  [[ $pad -lt 4 ]] && payload="${payload}$(printf '=%.0s' $(seq 1 "$pad"))"
  payload=$(echo "$payload" | base64 -d 2>/dev/null)

  local sub
  sub=$(echo "$payload" | jq -r '.sub')
  assert_equals "system" "$sub"
}

# -- error cases --

function test_missing_private_key_returns_error() {
  local output
  # Override PRIVATE_KEY_PATH by temporarily renaming the key
  local key="apps/api/tests/assets/auth/test_private_key.pem"
  mv "$key" "${key}.bak"
  output=$($SCRIPT 2>&1)
  local exit_code=$?
  mv "${key}.bak" "$key"
  assert_equals "1" "$exit_code"
  assert_contains "test private key not found" "$output"
}
