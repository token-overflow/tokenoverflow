#!/usr/bin/env bash

function setup_opentofu() {
  tofuenv install
  TOFUENV_TOFU_VERSION="$(cat .opentofu-version)"
  export TOFUENV_TOFU_VERSION
  tofuenv use "$TOFUENV_TOFU_VERSION"
}

function tg() {
  local action="$1"
  local env="$2"

  if [ -z "$action" ] || [ -z "$env" ]; then
    echo "Usage: tg <plan|apply> <global/dev/prod>" >&2
    return 1
  fi

  case "$env" in
    global) aws_profile="tokenoverflow-root-admin" ;;
    dev) aws_profile="tokenoverflow-dev-admin" ;;
    prod) aws_profile="tokenoverflow-prod-admin" ;;
    *)
      echo "Unknown environment: $env (expected: global/dev/prod)" >&2
      return 1
      ;;
  esac

  if ! aws sts get-caller-identity --profile "$aws_profile" >/dev/null 2>&1; then
    echo "AWS SSO session expired or missing — logging in..."
    aws sso login --profile "$aws_profile" || return 1
  fi

  export AWS_PROFILE=$aws_profile

  cd "infra/terraform/live/$env" || return 1
  terragrunt run --all "$action"
  cd - >/dev/null || return 1
}
