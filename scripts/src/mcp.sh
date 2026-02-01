#!/usr/bin/env bash

function claude_plugin() {
  claude --dangerously-skip-permissions --ide --plugin-dir ./integrations/claude
}

function claude_local() {
  local env_file="bruno/tokenoverflow/collections/api/environments/local.yml"

  local base_url
  base_url=$(yq '.variables[] | select(.name == "base_url") | .value' "$env_file")

  local access_token
  access_token=$(yq '.variables[] | select(.name == "access_token") | .value' "$env_file")

  # Copy the full plugin and override .mcp.json with a local-only version that injects a
  # static Bearer token. The production .mcp.json uses OAuth (no static headers) because
  # the MCP SDK merges static headers AFTER the OAuth token, which would overwrite it
  # with an empty value.
  local plugin_dir="./integrations/claude"
  local tmp_dir
  tmp_dir=$(mktemp -d)
  cp -R "$plugin_dir"/. "$tmp_dir"

  cat > "$tmp_dir/.mcp.json" <<JSON
{
  "mcpServers": {
    "tokenoverflow": {
      "type": "http",
      "url": "${base_url}/mcp",
      "headers": {
        "Authorization": "Bearer ${access_token}"
      }
    }
  }
}
JSON

  claude --dangerously-skip-permissions --ide --plugin-dir "$tmp_dir"
  rm -rf "$tmp_dir"
}
