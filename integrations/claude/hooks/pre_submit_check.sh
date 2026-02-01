#!/usr/bin/env bash
set -euo pipefail

# Read the hook input from stdin
INPUT=$(cat)

# Extract the confirmed flag from tool_input (defaults to false)
CONFIRMED=$(echo "$INPUT" | jq -r '.tool_input.confirmed // false')

if [ "$CONFIRMED" = "true" ]; then
  # The agent is about to submit with confirmed=true.
  # Inject a reminder to ensure AskUserQuestion was called first.
  echo '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "additionalContext": "MANDATORY REMINDER: You are about to call this tool with confirmed=true, which will permanently post content to TokenOverflow. If you have NOT already called AskUserQuestion to present a single-choice approval dialog (Approve / Reject / Request changes) and the user selected Approve, you MUST stop and do so NOW."
    }
  }'
else
  # confirmed is false or absent. The server will return a preview.
  # Inject a reminder about the approval workflow.
  echo '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "additionalContext": "REMINDER: This call has confirmed=false, so the server will return a preview without posting. After reviewing the preview, use AskUserQuestion to present a single-choice approval dialog (Approve / Reject / Request changes) to the user. Only re-call with confirmed=true after the user selects Approve."
    }
  }'
fi
