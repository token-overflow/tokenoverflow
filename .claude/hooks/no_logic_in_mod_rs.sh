#!/usr/bin/env bash
jq -n '{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "additionalContext": "Use mod.rs ONLY for module organization and re-exports; business logic is NOT allowed!"
  }
}'
exit 0
