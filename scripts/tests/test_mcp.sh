#!/usr/bin/env bash
# Tests for mcp.sh

set_up_before_script() {
  # shellcheck source=scripts/src/mcp.sh
  source "scripts/src/mcp.sh"
}

set_up() {
  TOKENOVERFLOW_MOCK_LOG=""
  TOKENOVERFLOW_MOCK_MCP_JSON=""

  # Mock yq to return controlled values
  yq() {
    if [[ "$*" == *"base_url"* ]]; then
      echo "http://localhost:9999"
    elif [[ "$*" == *"access_token"* ]]; then
      echo "mock-jwt-token"
    fi
  }
  export -f yq

  # Mock claude to capture args and read the temp .mcp.json before it
  # gets cleaned up.
  claude() {
    TOKENOVERFLOW_MOCK_LOG+="claude $*;"
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --plugin-dir)
          if [[ -f "$2/.mcp.json" ]]; then
            TOKENOVERFLOW_MOCK_MCP_JSON=$(cat "$2/.mcp.json")
          fi
          shift 2
          ;;
        *)
          shift
          ;;
      esac
    done
  }
  export -f claude
}

tear_down() {
  unset TOKENOVERFLOW_MOCK_LOG TOKENOVERFLOW_MOCK_MCP_JSON
}

# --- claude_plugin ---

function test_claude_plugin_passes_correct_flags() {
  claude_plugin
  assert_contains "--dangerously-skip-permissions" "$TOKENOVERFLOW_MOCK_LOG"
  assert_contains "--ide" "$TOKENOVERFLOW_MOCK_LOG"
  assert_contains "--plugin-dir ./integrations/claude" "$TOKENOVERFLOW_MOCK_LOG"
}

# --- claude_local ---

function test_claude_local_launches_claude() {
  claude_local
  assert_contains "claude" "$TOKENOVERFLOW_MOCK_LOG"
  assert_contains "--dangerously-skip-permissions" "$TOKENOVERFLOW_MOCK_LOG"
  assert_contains "--ide" "$TOKENOVERFLOW_MOCK_LOG"
}

function test_claude_local_generates_mcp_json_with_url() {
  claude_local
  assert_contains "http://localhost:9999/mcp" "$TOKENOVERFLOW_MOCK_MCP_JSON"
}

function test_claude_local_generates_mcp_json_with_bearer_token() {
  claude_local
  assert_contains "Bearer mock-jwt-token" "$TOKENOVERFLOW_MOCK_MCP_JSON"
}

function test_claude_local_generates_mcp_json_without_oauth() {
  claude_local
  assert_not_contains "clientId" "$TOKENOVERFLOW_MOCK_MCP_JSON"
}
