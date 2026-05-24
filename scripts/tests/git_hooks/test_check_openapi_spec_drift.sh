#!/usr/bin/env bash

HOOK_SCRIPT=""
ORIG_DIR=""
ORIG_PATH=""
TEST_DIR=""

set_up_before_script() {
  HOOK_SCRIPT="$(pwd)/scripts/src/git_hooks/check_openapi_spec_drift.sh"
}

set_up() {
  TEST_DIR=$(mktemp -d)
  ORIG_DIR="$(pwd)"
  ORIG_PATH="$PATH"
  # Defensively drop pre-commit's git env so any future test additions that
  # touch git in the sandbox cannot write to the parent repo.
  unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE
  # Mirror the layout the hook expects: api.sh next to the hook, plus the
  # committed spec at apps/api/openapi.json.
  mkdir -p "$TEST_DIR/scripts/src/git_hooks"
  mkdir -p "$TEST_DIR/apps/api"
  cp "$ORIG_DIR/scripts/src/api.sh" "$TEST_DIR/scripts/src/api.sh"
  cp "$HOOK_SCRIPT" "$TEST_DIR/scripts/src/git_hooks/check_openapi_spec_drift.sh"
  printf '{"openapi":"3.1.0","info":{"title":"x"}}\n' > "$TEST_DIR/apps/api/openapi.json"
  mkdir -p "$TEST_DIR/bin"
  export PATH="$TEST_DIR/bin:$PATH"
  cd "$TEST_DIR" || exit 1
}

tear_down() {
  export PATH="$ORIG_PATH"
  cd "$ORIG_DIR" || exit 1
  rm -rf "$TEST_DIR"
}

# cargo stub helpers. Each test calls one to install a stub that handles the
# `cargo run ... -- --openapi-json` invocation gen_api_spec issues.

stub_cargo_matching() {
  cat >bin/cargo <<'EOF'
#!/usr/bin/env bash
# Echo bytes identical to the committed spec so diff sees no drift.
printf '{"openapi":"3.1.0","info":{"title":"x"}}\n'
EOF
  chmod +x bin/cargo
}

stub_cargo_drift() {
  cat >bin/cargo <<'EOF'
#!/usr/bin/env bash
# Echo a different payload so diff trips the drift branch.
printf '{"openapi":"3.1.0","info":{"title":"changed"}}\n'
EOF
  chmod +x bin/cargo
}

stub_cargo_drift_huge() {
  # Force a long diff so we can assert head -80 actually caps the output.
  cat >bin/cargo <<'EOF'
#!/usr/bin/env bash
printf '{"openapi":"3.1.0","info":{"title":"x"},"paths":{'
for i in $(seq 1 500); do
  printf '"/r%d":{"get":{"responses":{"200":{"description":"r%d"}}}},' "$i" "$i"
done
printf '"/last":{"get":{"responses":{"200":{"description":"last"}}}}}}\n'
EOF
  chmod +x bin/cargo
}

stub_cargo_broken() {
  cat >bin/cargo <<'EOF'
#!/usr/bin/env bash
echo "error[E0425]: cannot find value `nope` in this scope" >&2
exit 1
EOF
  chmod +x bin/cargo
}

function test_passes_when_committed_spec_matches_source() {
  stub_cargo_matching
  "./scripts/src/git_hooks/check_openapi_spec_drift.sh"
  assert_exit_code "0"
}

function test_fails_when_committed_spec_is_stale() {
  stub_cargo_drift
  local output
  output=$("./scripts/src/git_hooks/check_openapi_spec_drift.sh" 2>&1)
  assert_exit_code "1"
  assert_contains "is stale" "$output"
  assert_contains "gen_api_spec" "$output"
  assert_contains "changed" "$output"
}

function test_diff_output_is_capped_at_80_lines() {
  stub_cargo_drift_huge
  local stderr_lines
  stderr_lines=$("./scripts/src/git_hooks/check_openapi_spec_drift.sh" 2>&1 1>/dev/null | wc -l)
  # 80-line cap on the diff hunk plus 4 lines of remediation header.
  # Anything significantly larger means the head -80 redirect is bypassed.
  assert_less_or_equal_than 90 "$stderr_lines"
}

function test_fails_loud_when_cargo_build_broken() {
  stub_cargo_broken
  local output
  output=$("./scripts/src/git_hooks/check_openapi_spec_drift.sh" 2>&1)
  assert_exit_code "1"
  assert_contains "cargo failed" "$output"
  assert_contains "E0425" "$output"
  assert_not_contains "stale" "$output"
}

function test_no_temp_files_leak() {
  # Confine mktemp to a sandbox we own so we can audit leftovers across both
  # the success and failure paths.
  local tmp_sandbox
  tmp_sandbox="$TEST_DIR/tmp_sandbox"
  mkdir -p "$tmp_sandbox"
  TMPDIR="$tmp_sandbox" stub_cargo_matching
  TMPDIR="$tmp_sandbox" "./scripts/src/git_hooks/check_openapi_spec_drift.sh" >/dev/null 2>&1
  assert_equals "" "$(find "$tmp_sandbox" -name 'openapi-spec.*' -print)"

  TMPDIR="$tmp_sandbox" stub_cargo_drift
  TMPDIR="$tmp_sandbox" "./scripts/src/git_hooks/check_openapi_spec_drift.sh" >/dev/null 2>&1
  assert_equals "" "$(find "$tmp_sandbox" -name 'openapi-spec.*' -print)"

  TMPDIR="$tmp_sandbox" stub_cargo_broken
  TMPDIR="$tmp_sandbox" "./scripts/src/git_hooks/check_openapi_spec_drift.sh" >/dev/null 2>&1
  assert_equals "" "$(find "$tmp_sandbox" -name 'openapi-spec.*' -print)"
}
