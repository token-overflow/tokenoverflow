#!/usr/bin/env bash
#
# Generates test JWTs for local development and MCP testing.
# Signs with the test private key checked into the repo -- safe because the
# test issuer and audience are rejected by production.
#
# Usage:
#   ./scripts/src/dev_token.sh                     # 1h token
#   ./scripts/src/dev_token.sh --expiry 10y        # 10-year token (for Bruno/MCP)
#   ./scripts/src/dev_token.sh --sub user_custom   # custom subject
#
# Requires: openssl, jq (both in Brewfile)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

PRIVATE_KEY_PATH="$PROJECT_ROOT/apps/api/tests/assets/auth/test_private_key.pem"
CONFIG_PATH="$PROJECT_ROOT/apps/api/config/local.toml"

# Defaults
SUB="system"
EXPIRY="1h"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sub)
      SUB="$2"
      shift 2
      ;;
    --expiry)
      EXPIRY="$2"
      shift 2
      ;;
    --help|-h)
      echo "Usage: $0 [--sub <subject>] [--expiry <duration>]"
      echo ""
      echo "Options:"
      echo "  --sub      Subject claim (default: system)"
      echo "  --expiry   Token lifetime: 1h, 24h, 10y, etc. (default: 1h)"
      echo ""
      echo "Examples:"
      echo "  $0                          # 1-hour token"
      echo "  $0 --expiry 10y             # 10-year token for Bruno/MCP"
      echo "  $0 --sub user_custom --expiry 24h"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

if [[ ! -f "$PRIVATE_KEY_PATH" ]]; then
  echo "Error: test private key not found at $PRIVATE_KEY_PATH" >&2
  exit 1
fi

# Parse issuer and audience from local.toml
# audiences is a TOML array, extract the first element
ISSUER=$(grep '^issuer' "$CONFIG_PATH" | head -1 | sed 's/.*= *"\(.*\)"/\1/')
AUDIENCE=$(grep '^audiences' "$CONFIG_PATH" | head -1 | sed 's/.*\["\([^"]*\)".*/\1/')
KID="test-key-1"

if [[ -z "$ISSUER" || -z "$AUDIENCE" ]]; then
  echo "Error: could not parse issuer or audience from $CONFIG_PATH" >&2
  exit 1
fi

# Convert expiry to seconds
parse_expiry() {
  local val="$1"
  local num="${val%[a-zA-Z]*}"
  local unit="${val##*[0-9]}"

  case "$unit" in
    s) echo "$num" ;;
    m) echo $((num * 60)) ;;
    h) echo $((num * 3600)) ;;
    d) echo $((num * 86400)) ;;
    y) echo $((num * 365 * 86400)) ;;
    *)
      echo "Error: unsupported expiry unit '$unit' (use s, m, h, d, y)" >&2
      exit 1
      ;;
  esac
}

EXPIRY_SECS=$(parse_expiry "$EXPIRY")
NOW=$(date +%s)
EXP=$((NOW + EXPIRY_SECS))

# Base64url encode (no padding, URL-safe)
b64url() {
  openssl base64 -e -A | tr '+/' '-_' | tr -d '='
}

# Build JWT header
HEADER=$(jq -nc --arg kid "$KID" '{
  "alg": "RS256",
  "typ": "JWT",
  "kid": $kid
}')

# Build JWT payload
PAYLOAD=$(jq -nc \
  --arg sub "$SUB" \
  --arg iss "$ISSUER" \
  --arg aud "$AUDIENCE" \
  --argjson iat "$NOW" \
  --argjson exp "$EXP" \
  '{
    "sub": $sub,
    "iss": $iss,
    "aud": $aud,
    "iat": $iat,
    "exp": $exp
  }')

HEADER_B64=$(echo -n "$HEADER" | b64url)
PAYLOAD_B64=$(echo -n "$PAYLOAD" | b64url)

# Sign with RSA-SHA256
SIGNATURE=$(echo -n "${HEADER_B64}.${PAYLOAD_B64}" | \
  openssl dgst -sha256 -sign "$PRIVATE_KEY_PATH" | b64url)

echo "${HEADER_B64}.${PAYLOAD_B64}.${SIGNATURE}"
