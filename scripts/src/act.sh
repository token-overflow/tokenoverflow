#!/usr/bin/env bash

function act_terraform() {
  local event="${1:-push}"
  act "$event" \
    -W .github/workflows/terraform.yml \
    -e ".github/act/event_${event}_main.json" \
    --secret-file .act.secrets
}

function act_deploy() {
  local event="${1:-push}"
  act "$event" \
    -W .github/workflows/deploy_api.yml \
    -e ".github/act/event_${event}_main.json" \
    --secret-file .act.secrets
}
