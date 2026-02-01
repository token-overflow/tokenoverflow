#!/usr/bin/env bash

# Brew

function setup_brew() {
  echo "🍺 Installing Homebrew."
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/dc89d02c0107d688e089c1683dcda3401719f1f8/install.sh)"
  echo "🍺 Installing formulas from the bundle."
  brew bundle install --file=Brewfile
}

# Rust

function setup_rust() {
  echo "🦀 Setting up Rust toolchain."
  rustup-init -y --no-modify-path
  # shellcheck source=/dev/null
  source "${HOME}/.cargo/env"
  rustup toolchain install nightly
  rustup component add llvm-tools-preview
  cargo install cargo-llvm-cov
}

# Javascript

function setup_javascript() {
  echo "📦 Setting up JavaScript toolchain."
  bun run turbo
}

# Git

function setup_precommit() {
  echo "🔧 Installing pre-commit hooks."
  prek install
}

# Environment variables

function setup_env() {
  echo "🔧 Setting up environment variables."
  case ":$PATH:" in
    *":/opt/homebrew/opt/postgresql@18/bin:"*) ;;
    *) export PATH="/opt/homebrew/opt/postgresql@18/bin:$PATH" ;;
  esac
}

# AWS

function setup_aws() {
  echo "☁️ Setting up AWS CLI."
  upsert_config_block "${HOME}/.aws/config" "
[sso-session tokenoverflow]
sso_start_url = https://d-906600a5bd.awsapps.com/start
sso_region = us-east-1
sso_registration_scopes = sso:account:access

[profile tokenoverflow-dev-admin]
sso_session = tokenoverflow
sso_account_id = 871610744185
sso_role_name = AdministratorAccess
region = us-east-1

[profile tokenoverflow-prod-admin]
sso_session = tokenoverflow
sso_account_id = 591120835062
sso_role_name = AdministratorAccess
region = us-east-1

[profile tokenoverflow-root-admin]
sso_session = tokenoverflow
sso_account_id = 058170691494
sso_role_name = AdministratorAccess
region = us-east-1
"
}

# Git

function setup_git_lfs() {
  echo "🔧 Setting up Git LFS."
  git lfs install
  git lfs pull
}

# Complete Setup

function setup() {
  setup_brew
  setup_opentofu
  setup_rust
  setup_javascript
  setup_precommit
  setup_env
  setup_aws
  setup_git_lfs
}
