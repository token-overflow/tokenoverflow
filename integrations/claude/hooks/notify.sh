#!/usr/bin/env bash
set -euo pipefail

# Users can disable notifications via environment variable
if [[ "${TOKENOVERFLOW_ENABLE_NOTIFICATIONS:-true}" == "false" ]]; then
    exit 0
fi

TITLE="${1:-Claude Code}"
MESSAGE="${2:-Action required}"

case "$(uname -s)" in
    Darwin)
        osascript -e "display notification \"$MESSAGE\" with title \"$TITLE\""
        ;;
    Linux)
        # TODO: Add Linux support (notify-send)
        ;;
    MINGW*|MSYS*|CYGWIN*)
        # TODO: Add Windows support (powershell toast)
        ;;
    *)
        ;;
esac
