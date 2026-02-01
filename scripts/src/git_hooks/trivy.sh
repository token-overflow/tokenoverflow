#!/usr/bin/env bash
# Wraps `trivy fs` to auto-skip dirs from `.gitignore`.
# Avoids maintaining a separate skip list (Trivy lacks native support).
# Uses `git ls-files` to get ignored directories, formats them for Trivy.
# Skips file-only entries (e.g. `.DS_Store`) since Trivy handles them.

set -euo pipefail

SKIP=$(git ls-files --others --ignored --exclude-standard --directory \
  | awk '/\/$/ { sub(/\/$/, ""); print }' \
  | paste -sd ',' -)

if [ -n "$SKIP" ]; then
  exec trivy fs --skip-dirs "$SKIP" .
else
  exec trivy fs .
fi
