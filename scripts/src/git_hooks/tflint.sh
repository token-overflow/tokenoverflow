#!/usr/bin/env bash

CONFIG="$(pwd)/infra/terraform/.tflint.hcl"
MODULES="$(pwd)/infra/terraform/modules/"

tflint --config="${CONFIG}" --chdir="${MODULES}" --init
tflint --config="${CONFIG}" --chdir="${MODULES}" --recursive
