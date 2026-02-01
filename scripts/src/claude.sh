#!/usr/bin/env bash
# Entrypoint for running IDE/editor's Claude Code integration with the local plugin.
#
# VSCode:
#   "claudeCode.claudeProcessWrapper": "/Users/berkay/projects/tokenoverflow/.worktrees/tokenoverflow-1/scripts/src/claude.sh"

claude \
  --ide \
  --chrome \
  --dangerously-skip-permissions \
  --plugin-dir "${PROJECTS}/tokenoverflow/integrations/claude" \
  "$@"

